use std::{io, path::Path, sync::Arc};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use server_common::{
    IndependentCreateControl, SetupAdmission, SetupAdmissionError, setup_files, setup_origin,
    setup_origin_guard,
};
use setup::join::JoinHost;
use tokio::sync::watch;
use tower_http::cors::CorsLayer;
use utoipa::{
    OpenApi, ToSchema,
    openapi::{
        OpenApi as Document,
        security::{ApiKey, ApiKeyValue, SecurityScheme},
    },
};

pub const ADMISSION: &str = "node-admission";
pub const OWNERS: [&str; 3] = ["idp", "device", "storage"];

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
    binding: String,
    ui: HeaderValue,
    pm: HeaderValue,
    base: String,
}

#[derive(Clone)]
pub enum Completion {
    Starting,
    Create {
        owners: [IndependentCreateControl; 3],
        management: watch::Receiver<Option<Router>>,
    },
    Join(Arc<JoinHost>),
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
enum Descriptor {
    AwaitingChoice,
    Starting,
    Creating,
    Joining,
    Blocked,
    Ready { idp: String, storage: String },
}

impl NodeSetup {
    pub fn open(root: &Path, ui: &str, pm: &str, base: &str) -> io::Result<Self> {
        let path = root.join(ADMISSION);
        let mut create = false;
        for owner in OWNERS {
            create |= setup_files::present(&root.join(owner))?
                || setup_files::present(&root.join(owner).with_extension("setup-host"))?;
        }
        let join = setup_files::present(&root.join("join"))?;
        if create || join {
            setup_files::require_private(&path, true).map_err(|_error| {
                io::Error::other("retained setup has no Node choice; preserve it and choose a fresh setup data root")
            })?;
        }
        let admission = Arc::new(SetupAdmission::open(&path)?);
        let binding = format!("{base} {ui} {pm}");
        let retained = admission.host_intent().map_err(io::Error::other)?;
        let choice = match retained.as_deref() {
            None if !create && !join => None,
            Some(intent) if !join && intent == choice_intent(Choice::Create, &binding) => {
                Some(Choice::Create)
            }
            Some(intent) if !create && intent == choice_intent(Choice::Join, &binding) => {
                Some(Choice::Join)
            }
            _ => {
                return Err(io::Error::other(
                    "retained Node choice or HTTPS origins disagree",
                ));
            }
        };
        if choice.is_none() {
            println!(
                "Protected Node setup-code retrieval path: {}",
                path.join("state").display()
            );
        }
        Ok(Self {
            admission,
            choice: watch::Sender::new(choice),
            binding,
            ui: setup_origin(ui)?,
            pm: setup_origin(pm)?,
            base: base.into(),
        })
    }

    pub fn choice(&self) -> watch::Receiver<Option<Choice>> {
        self.choice.subscribe()
    }

    pub fn router(&self, completion: Completion) -> Router {
        let reads = Router::new()
            .route("/setup/node", get(describe))
            .with_state((self.clone(), completion))
            .layer(
                CorsLayer::new()
                    .allow_origin([self.ui.clone(), self.pm.clone()])
                    .allow_methods([Method::GET]),
            );
        let writes = Router::new()
            .route("/setup/node/choice", post(select))
            .layer(
                CorsLayer::new()
                    .allow_origin(self.ui.clone())
                    .allow_methods([Method::POST])
                    .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]),
            )
            .layer(middleware::from_fn_with_state(
                self.ui.clone(),
                setup_origin_guard,
            ))
            .with_state(self.clone());
        reads.merge(writes)
    }
}

#[utoipa::path(get, path = "/setup/node", operation_id = "node_setup_status", responses(
    (status = 200, body = Descriptor, description = "Current Node setup. Only verified completion publishes the selected service bases."),
    (status = 503, description = "The Node setup runtime is unavailable")
))]
async fn describe(State((node, completion)): State<(NodeSetup, Completion)>) -> Response {
    let state = match completion {
        Completion::Starting => {
            if node.choice.borrow().is_some() {
                Descriptor::Starting
            } else {
                Descriptor::AwaitingChoice
            }
        }
        Completion::Create {
            owners: [idp, device, storage],
            management,
        } => {
            let management_ready = management.borrow().is_some();
            let (idp, device, storage) = tokio::join!(
                idp.verified_intent(),
                device.verified_intent(),
                storage.verified_intent()
            );
            match (idp, device, storage) {
                (Some(idp), Some(device), Some(storage))
                    if management_ready
                        && idp == device
                        && idp == storage
                        && idp.idp == format!("{}/idp", node.base)
                        && idp.device == format!("{}/device", node.base)
                        && idp.storage == format!("{}/storage", node.base) =>
                {
                    Descriptor::Ready {
                        idp: idp.idp,
                        storage: idp.storage,
                    }
                }
                _ => Descriptor::Creating,
            }
        }
        Completion::Join(join) => match join.approved_bases().await {
            Ok(Some((idp, storage))) => Descriptor::Ready { idp, storage },
            Ok(None) => Descriptor::Joining,
            Err(_error) => Descriptor::Blocked,
        },
    };
    ([(header::CACHE_CONTROL, "no-store")], Json(state)).into_response()
}

fn choice_intent(choice: Choice, binding: &str) -> String {
    let mode = match choice {
        Choice::Create => "create",
        Choice::Join => "join",
    };
    format!("node-v1:{mode} {binding}")
}

#[utoipa::path(post, path = "/setup/node/choice", operation_id = "select_node_setup", request_body = Choice,
    security(("node_setup_code" = [])),
    responses(
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
    node.choice.send_replace(Some(choice));
    Ok(StatusCode::NO_CONTENT)
}

/// Returns the contract for the local Node setup choice and descriptor.
#[must_use]
pub fn node_setup_openapi() -> Document {
    #[derive(OpenApi)]
    #[openapi(paths(describe, select), components(schemas(Choice, Descriptor)))]
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
