use std::{io, sync::Arc};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use server_common::SetupAdmission;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use utoipa::ToSchema;

use crate::{
    node_runtime::{CreateRecord, NodeRuntime},
    node_setup::{Choice, Descriptor, LoginClient, NodeSetup},
    setup_store::SetupStore,
};

const RECORD: &str = "create";

#[derive(Clone)]
pub struct NodeCreate {
    node: NodeSetup,
    admission: Arc<SetupAdmission>,
    store: SetupStore,
    state: Arc<Mutex<CreateState>>,
    commands: mpsc::Sender<(String, CancellationToken)>,
}

struct CreateState {
    record: Option<CreateRecord>,
    active: Option<CancellationToken>,
    router: Option<Router>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRequest {
    administrator: String,
    password: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct CreateRecovery {
    administrator: String,
    password_required: bool,
    operation_nonce: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CancelRequest {
    operation_nonce: String,
}

impl NodeCreate {
    pub async fn open(
        node: NodeSetup,
        store: SetupStore,
    ) -> io::Result<(Self, mpsc::Receiver<(String, CancellationToken)>)> {
        let record = store.read(RECORD).await?;
        let (commands, receiver) = mpsc::channel(1);
        Ok((
            Self {
                admission: node.admission(),
                node,
                store,
                commands,
                state: Arc::new(Mutex::new(CreateState {
                    record,
                    active: None,
                    router: None,
                })),
            },
            receiver,
        ))
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/setup/node/create", post(create))
            .route("/setup/node/create/intent", post(recover))
            .route("/setup/node/create/cancel", post(cancel))
            .layer(DefaultBodyLimit::max(32 * 1024))
            .with_state(self.clone())
    }

    pub fn owner_routes(&self) -> Router {
        let state = self.state.clone();
        Router::new().fallback(move |request: Request| async move {
            let router = state.lock().await.router.clone();
            match router {
                Some(router) => router
                    .oneshot(request)
                    .await
                    .expect("infallible Node router"),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        })
    }

    pub async fn run(
        self,
        runtime: NodeRuntime,
        mut commands: mpsc::Receiver<(String, CancellationToken)>,
        shutdown: CancellationToken,
    ) -> io::Result<()> {
        let restore = {
            let state = self.state.lock().await;
            let restore = state
                .record
                .as_ref()
                .is_some_and(|record| record.device.is_some());
            if !restore && state.active.is_none() {
                self.node.publish(Descriptor::Creating { active: false });
            }
            restore
        };
        if restore {
            if let Err(error) = self.build(&runtime, None, &shutdown, &shutdown).await {
                eprintln!("Cannot open retained Node Installation: {error}");
                self.node.publish(Descriptor::Blocked { create: true });
            }
        }
        loop {
            let (password, cancelled) = tokio::select! {
                biased;
                () = shutdown.cancelled() => break,
                command = commands.recv() => match command { Some(command) => command, None => break },
            };
            if let Err(error) = self
                .build(&runtime, Some(&password), &cancelled, &shutdown)
                .await
            {
                let mut state = self.state.lock().await;
                state.active = None;
                if error.kind() == io::ErrorKind::Interrupted {
                    self.node.publish(Descriptor::Creating { active: false });
                } else {
                    eprintln!("Cannot complete Node Create: {error}");
                    self.node.publish(Descriptor::Blocked { create: true });
                }
            }
        }
        Ok(())
    }

    async fn build(
        &self,
        runtime: &NodeRuntime,
        password: Option<&str>,
        cancelled: &CancellationToken,
        shutdown: &CancellationToken,
    ) -> io::Result<()> {
        let mut record: CreateRecord = self
            .store
            .read(RECORD)
            .await?
            .ok_or_else(|| io::Error::other("Node Create record is missing"))?;
        let router = runtime
            .build(&mut record, password, cancelled, shutdown)
            .await?;
        let mut state = self.state.lock().await;
        if cancelled.is_cancelled() || shutdown.is_cancelled() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if password.is_some() {
            self.store.write(RECORD, &record).await?;
        }
        self.node.retire(Choice::Create)?;
        let client_id = record.login.to_string();
        state.router = Some(router);
        state.active = None;
        state.record = Some(record);
        self.node.publish(Descriptor::Ready {
            idp: format!("{}/idp", runtime.base),
            storage: format!("{}/storage", runtime.base),
            login_client: Some(LoginClient {
                client_id,
                scopes: vec!["openid"],
            }),
        });
        Ok(())
    }

    fn authorize(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        self.admission
            .check(headers)
            .map_err(|error| error.into_response().status())
    }

    async fn start(&self, request: CreateRequest) -> Result<StatusCode, StatusCode> {
        let mut state = self.state.lock().await;
        if request.administrator.trim().is_empty()
            || request.administrator.len() > 256
            || request.administrator.chars().any(char::is_control)
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        if let Some(record) = &state.record {
            if record.device.is_some() || record.administrator != request.administrator {
                return Err(StatusCode::CONFLICT);
            }
        }
        let password = request
            .password
            .filter(|password| (8..=4096).contains(&password.len()) && !password.trim().is_empty())
            .ok_or(StatusCode::BAD_REQUEST)?;
        if let Some(active) = &state.active {
            return if active.is_cancelled() {
                Err(StatusCode::CONFLICT)
            } else {
                Ok(StatusCode::ACCEPTED)
            };
        }
        if state.record.is_none() {
            let record = CreateRecord::new(request.administrator)
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            self.store
                .write(RECORD, &record)
                .await
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            state.record = Some(record);
        }
        let cancelled = CancellationToken::new();
        self.commands
            .try_send((password, cancelled.clone()))
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        state.active = Some(cancelled);
        self.node.publish(Descriptor::Creating { active: true });
        Ok(StatusCode::ACCEPTED)
    }
}

fn no_store(result: Result<impl IntoResponse, StatusCode>) -> Response {
    let mut response = result.into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("static header"),
    );
    response
}

#[utoipa::path(post, path = "/setup/node/create", operation_id = "create_node_installation", request_body = CreateRequest,
    security(("node_setup_code" = [])), responses(
        (status = 202, description = "Create accepted; observe Node setup events for completion"),
        (status = 400, description = "Invalid administrator details"),
        (status = 401, description = "Protected Node code missing or invalid"),
        (status = 403, description = "Browser origin not allowed"),
        (status = 409, description = "Create conflicts with retained setup"),
        (status = 503, description = "Setup unavailable")
    ))]
pub async fn create(
    State(node): State<NodeCreate>,
    headers: HeaderMap,
    Json(request): Json<CreateRequest>,
) -> Response {
    no_store(
        async {
            node.authorize(&headers)?;
            node.start(request).await
        }
        .await,
    )
}

#[utoipa::path(post, path = "/setup/node/create/intent", operation_id = "recover_node_create",
    security(("node_setup_code" = [])), responses(
        (status = 200, body = CreateRecovery, description = "Exact retained Create details, without credentials"),
        (status = 204, description = "No Create operation is retained"),
        (status = 401, description = "Protected Node code missing or invalid"),
        (status = 403, description = "Browser origin not allowed"),
        (status = 503, description = "Setup unavailable")
    ))]
pub async fn recover(State(node): State<NodeCreate>, headers: HeaderMap) -> Response {
    no_store(
        async {
            node.authorize(&headers)?;
            let state = node.state.lock().await;
            Ok(match &state.record {
                Some(record) => Json(CreateRecovery {
                    administrator: record.administrator.clone(),
                    password_required: record.device.is_none(),
                    operation_nonce: record.nonce.clone(),
                })
                .into_response(),
                None => StatusCode::NO_CONTENT.into_response(),
            })
        }
        .await,
    )
}

#[utoipa::path(post, path = "/setup/node/create/cancel", operation_id = "cancel_node_create", request_body = CancelRequest,
    security(("node_setup_code" = [])), responses(
        (status = 204, description = "Cancellation signaled; retained state is not rolled back"),
        (status = 401, description = "Protected Node code missing or invalid"),
        (status = 403, description = "Browser origin not allowed"),
        (status = 409, description = "No matching incomplete Create operation"),
        (status = 503, description = "Setup unavailable")
    ))]
pub async fn cancel(
    State(node): State<NodeCreate>,
    headers: HeaderMap,
    Json(request): Json<CancelRequest>,
) -> Response {
    no_store(
        async {
            node.authorize(&headers)?;
            let state = node.state.lock().await;
            if !state.record.as_ref().is_some_and(|record| {
                record.nonce == request.operation_nonce && record.device.is_none()
            }) {
                return Err(StatusCode::CONFLICT);
            }
            if let Some(active) = &state.active {
                active.cancel();
            }
            Ok(StatusCode::NO_CONTENT)
        }
        .await,
    )
}
