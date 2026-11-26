use std::{convert::Infallible, io, path::Path, sync::Arc};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware,
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use server_common::{SetupAdmission, SetupAdmissionError, setup_origin, setup_origin_guard};
use setup::join::JoinHost;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use utoipa::{
    OpenApi, ToSchema,
    openapi::{
        OpenApi as Document,
        security::{ApiKey, ApiKeyValue, SecurityScheme},
    },
};

use crate::{
    node_create::{self, CancelRequest, CreateRecovery, CreateRequest},
    setup_store::SetupStore,
};

pub const ADMISSION: &str = "node-admission";

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    Create,
    Join,
}

#[derive(Clone)]
pub struct NodeSetup {
    admission: Arc<SetupAdmission>,
    choice: watch::Sender<Option<Choice>>,
    status: watch::Sender<Descriptor>,
    join: watch::Sender<Option<Arc<JoinHost>>>,
    binding: String,
    ui: HeaderValue,
    pm: HeaderValue,
    shutdown: CancellationToken,
}

#[derive(Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Descriptor {
    AwaitingChoice,
    Starting,
    Creating {
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        active: bool,
    },
    Joining,
    Blocked {
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        create: bool,
    },
    Ready {
        idp: String,
        storage: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        login_client: Option<LoginClient>,
    },
}

#[derive(Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct LoginClient {
    pub client_id: String,
    pub scopes: Vec<&'static str>,
}

impl NodeSetup {
    pub fn open(
        root: &Path,
        ui: &str,
        pm: &str,
        base: &str,
        store: SetupStore,
        shutdown: CancellationToken,
    ) -> io::Result<Self> {
        let path = root.join(ADMISSION);
        let admission = Arc::new(SetupAdmission::open_with_history(&path, Arc::new(store))?);
        let binding = format!("{base} {ui} {pm}");
        let retained = admission.host_intent().map_err(io::Error::other)?;
        let choice = match retained.as_deref() {
            None => None,
            Some(intent) if intent == choice_intent(Choice::Create, &binding) => {
                Some(Choice::Create)
            }
            Some(intent) if intent == choice_intent(Choice::Join, &binding) => Some(Choice::Join),
            _ => {
                return Err(io::Error::other(
                    "retained Node choice or HTTPS origins disagree",
                ));
            }
        };
        if !admission.is_retired().map_err(io::Error::other)? {
            println!(
                "Protected Node setup-code retrieval path: {}",
                path.join("state").display()
            );
        }
        let initial = if choice.is_some() {
            Descriptor::Starting
        } else {
            Descriptor::AwaitingChoice
        };
        Ok(Self {
            admission,
            choice: watch::Sender::new(choice),
            status: watch::Sender::new(initial),
            join: watch::Sender::new(None),
            binding,
            ui: setup_origin(ui)?,
            pm: setup_origin(pm)?,
            shutdown,
        })
    }

    pub fn choice(&self) -> watch::Receiver<Option<Choice>> {
        self.choice.subscribe()
    }

    pub fn admission(&self) -> Arc<SetupAdmission> {
        Arc::clone(&self.admission)
    }

    pub fn publish(&self, status: Descriptor) {
        self.status.send_if_modified(|current| {
            if *current == status {
                return false;
            }
            *current = status;
            true
        });
    }

    pub fn joined(&self, join: Arc<JoinHost>) {
        self.join.send_replace(Some(join));
        self.publish(Descriptor::Joining);
    }

    pub fn router(&self, create: Router) -> Router {
        let reads = Router::new()
            .route("/setup/node", get(describe))
            .with_state(self.clone())
            .layer(
                CorsLayer::new()
                    .allow_origin([self.ui.clone(), self.pm.clone()])
                    .allow_methods([Method::GET]),
            );
        let events = Router::new()
            .route("/setup/node/events", get(events))
            .with_state(self.clone())
            .layer(
                CorsLayer::new()
                    .allow_origin(self.ui.clone())
                    .allow_methods([Method::GET]),
            )
            .layer(middleware::from_fn_with_state(
                self.ui.clone(),
                setup_origin_guard,
            ));
        let writes = Router::new()
            .route("/setup/node/choice", post(select))
            .with_state(self.clone())
            .merge(create)
            .layer(
                CorsLayer::new()
                    .allow_origin(self.ui.clone())
                    .allow_methods([Method::POST])
                    .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
            )
            .layer(middleware::from_fn_with_state(
                self.ui.clone(),
                setup_origin_guard,
            ));
        reads.merge(events).merge(writes)
    }

    async fn snapshot(&self, descriptor: Descriptor) -> Descriptor {
        let join = self.join.borrow().clone();
        if descriptor == Descriptor::Joining
            && let Some(join) = join
        {
            match join.approved_bases().await {
                Ok(Some((idp, storage))) => {
                    return Descriptor::Ready {
                        idp,
                        storage,
                        login_client: None,
                    };
                }
                Ok(None) => (),
                Err(_) => return Descriptor::Blocked { create: false },
            }
        }
        descriptor
    }
}

#[utoipa::path(get, path = "/setup/node", operation_id = "node_setup_status", responses(
    (status = 200, body = Descriptor, description = "Current Node setup. Only verified completion publishes the selected service bases."),
    (status = 503, description = "The Node setup runtime is unavailable")
))]
async fn describe(State(node): State<NodeSetup>) -> Response {
    let descriptor = node.status.borrow().clone();
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(node.snapshot(descriptor).await),
    )
        .into_response()
}

#[utoipa::path(get, path = "/setup/node/events", operation_id = "node_setup_events", responses(
    (status = 200, content_type = "text/event-stream", body = Descriptor, description = "Server-sent message events. Each data field is a JSON setup descriptor. Sends the current snapshot, then pushes startup and Create changes. Join approval is read through the status endpoint."),
    (status = 403, description = "Browser origin not allowed"),
    (status = 503, description = "The Node setup runtime is unavailable")
))]
async fn events(State(node): State<NodeSetup>) -> Response {
    let mut status = node.status.subscribe();
    let stream = async_stream::stream! {
        loop {
            let descriptor = status.borrow_and_update().clone();
            let snapshot = tokio::select! {
                () = node.shutdown.cancelled() => break,
                snapshot = node.snapshot(descriptor) => snapshot,
            };
            yield Ok::<_, Infallible>(Event::default().json_data(snapshot).expect("serializable setup descriptor"));
            tokio::select! {
                () = node.shutdown.cancelled() => break,
                result = status.changed() => if result.is_err() { break; },
            }
        }
    };
    (
        [
            ("cache-control", "no-store, no-transform"),
            ("x-accel-buffering", "no"),
        ],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
        .into_response()
}

fn choice_intent(choice: Choice, binding: &str) -> String {
    let mode = match choice {
        Choice::Create => "create",
        Choice::Join => "join",
    };
    format!("node-v2:{mode} {binding}")
}

#[utoipa::path(post, path = "/setup/node/choice", operation_id = "select_node_setup", request_body = Choice,
    security(("node_setup_code" = [])), responses(
        (status = 204, description = "Choice is retained; this does not mean setup is complete"),
        (status = 401, description = "Protected Node code missing or invalid"),
        (status = 403, description = "Browser origin not allowed"),
        (status = 409, description = "A different choice is already retained"),
        (status = 503, description = "Protected retained state is unavailable")
    )
)]
async fn select(
    State(node): State<NodeSetup>,
    headers: HeaderMap,
    Json(choice): Json<Choice>,
) -> Result<StatusCode, SetupAdmissionError> {
    node.admission
        .bind(&headers, &choice_intent(choice, &node.binding))?;
    node.choice.send_if_modified(|current| {
        if current.is_some() {
            return false;
        }
        node.publish(Descriptor::Starting);
        *current = Some(choice);
        true
    });
    Ok(StatusCode::NO_CONTENT)
}

/// Returns the contract for local Node setup.
#[must_use]
pub fn node_setup_openapi() -> Document {
    #[derive(OpenApi)]
    #[openapi(
        paths(
            describe,
            events,
            select,
            node_create::create,
            node_create::recover,
            node_create::cancel
        ),
        components(schemas(
            Choice,
            Descriptor,
            LoginClient,
            CreateRequest,
            CreateRecovery,
            CancelRequest
        ))
    )]
    struct NodeOpenApi;
    let mut document = NodeOpenApi::openapi();
    document
        .components
        .get_or_insert_default()
        .add_security_scheme(
            "node_setup_code",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "Authorization",
                "Setup <protected Node code>",
            ))),
        );
    document
}

#[cfg(test)]
mod tests {
    use std::{
        fs::Permissions, future::poll_fn, os::unix::fs::PermissionsExt, pin::Pin, time::Duration,
    };

    use axum::{
        body::{Body, HttpBody},
        http::Request,
    };
    use db::EmbeddedDatabase;
    use tokio::time::timeout;
    use tower::ServiceExt;

    use super::{Descriptor, NodeSetup, Router, SetupStore};
    use tokio_util::sync::CancellationToken;

    async fn next_event(body: &mut Body) -> Option<String> {
        let frame = timeout(
            Duration::from_secs(2),
            poll_fn(|cx| Pin::new(&mut *body).poll_frame(cx)),
        )
        .await
        .expect("Node setup event deadline")?;
        let data = frame.expect("event frame").into_data().expect("event data");
        Some(String::from_utf8(data.to_vec()).expect("UTF-8 event"))
    }

    #[tokio::test]
    async fn events_send_a_snapshot_and_changes_then_close_on_shutdown() {
        let root = tempfile::Builder::new()
            .permissions(Permissions::from_mode(0o700))
            .tempdir()
            .expect("private Node root");
        let database = EmbeddedDatabase::open(root.path())
            .await
            .expect("Node database");
        let store = SetupStore::open(&database).await.expect("setup store");
        let stop = CancellationToken::new();
        let node = NodeSetup::open(
            root.path(),
            "https://app.test",
            "https://pm.test",
            "https://node.test",
            store,
            stop.clone(),
        )
        .expect("Node setup");
        let router = node.router(Router::new());
        let get = |origin| {
            Request::get("/setup/node/events")
                .header("origin", origin)
                .body(Body::empty())
                .expect("setup events request")
        };
        assert_eq!(
            router
                .clone()
                .oneshot(get("https://other.test"))
                .await
                .expect("events response")
                .status(),
            403
        );
        let response = router
            .oneshot(get("https://app.test"))
            .await
            .expect("events response");
        assert_eq!(response.headers()["content-type"], "text/event-stream");
        let mut body = response.into_body();
        assert_eq!(
            next_event(&mut body).await.as_deref(),
            Some("data: {\"state\":\"awaiting_choice\"}\n\n")
        );
        node.publish(Descriptor::Creating { active: true });
        assert_eq!(
            next_event(&mut body).await.as_deref(),
            Some("data: {\"state\":\"creating\",\"active\":true}\n\n")
        );
        stop.cancel();
        assert!(next_event(&mut body).await.is_none());
    }
}
