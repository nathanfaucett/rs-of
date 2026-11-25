use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use server_common::{
    IndependentCreateControl, IndependentCreateIntent, IndependentCreateRequest, SetupAdmission,
    random_capability,
};
use tokio::sync::Mutex;
use utoipa::ToSchema;

use crate::node_setup::OWNERS;

#[derive(Clone)]
pub struct NodeCreate {
    admission: Arc<SetupAdmission>,
    owners: [IndependentCreateControl; 3],
    base: String,
    operation: Arc<Mutex<()>>,
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
    pub fn new(
        admission: Arc<SetupAdmission>,
        owners: [IndependentCreateControl; 3],
        base: String,
    ) -> Self {
        Self {
            admission,
            owners,
            base,
            operation: Arc::new(Mutex::new(())),
        }
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/setup/node/create", post(create))
            .route("/setup/node/create/intent", post(recover))
            .route("/setup/node/create/cancel", post(cancel))
            .layer(DefaultBodyLimit::max(32 * 1024))
            .with_state(self.clone())
    }

    pub const fn owners(&self) -> &[IndependentCreateControl; 3] {
        &self.owners
    }

    pub fn matches_bases(&self, intent: &IndependentCreateIntent) -> bool {
        intent.issuer == intent.idp
            && OWNERS.iter().all(|owner| {
                intent.base(owner).ok() == Some(format!("{}/{owner}", self.base).as_str())
            })
    }

    async fn retained(&self) -> Result<(Option<IndependentCreateIntent>, [bool; 3]), StatusCode> {
        let mut exact = None;
        let mut retired = [false; 3];
        for (owner, retired) in self.owners.iter().zip(&mut retired) {
            let (intent, done) = owner
                .local_setup_state()
                .await
                .map_err(|_error| StatusCode::SERVICE_UNAVAILABLE)?;
            *retired = done;
            if let Some(intent) = intent {
                if !self.matches_bases(&intent)
                    || exact.as_ref().is_some_and(|exact| exact != &intent)
                {
                    return Err(StatusCode::CONFLICT);
                }
                exact = Some(intent);
            } else if done {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
        }
        Ok((exact, retired))
    }

    fn authorize(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        self.admission
            .check(headers)
            .map_err(|error| error.into_response().status())
    }

    async fn start(&self, request: CreateRequest) -> Result<StatusCode, StatusCode> {
        let _guard = self.operation.lock().await;
        let (retained, retired) = self.retained().await?;
        if retired == [true; 3] {
            return Err(StatusCode::CONFLICT);
        }
        let intent = if let Some(retained) = retained {
            if retained.administrator != request.administrator {
                return Err(StatusCode::CONFLICT);
            }
            retained
        } else {
            let nonce = random_capability().map_err(|_error| StatusCode::SERVICE_UNAVAILABLE)?;
            IndependentCreateIntent {
                purpose: "create".into(),
                operation_nonce: nonce[..32].into(),
                administrator: request.administrator,
                node: "My Node".into(),
                idp: format!("{}/idp", self.base),
                issuer: format!("{}/idp", self.base),
                device: format!("{}/device", self.base),
                storage: format!("{}/storage", self.base),
            }
        };
        intent
            .canonical()
            .map_err(|_error| StatusCode::BAD_REQUEST)?;
        if !retired[0]
            && request
                .password
                .as_ref()
                .is_none_or(|password| !(8..=4096).contains(&password.len()))
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        let mut password = request.password;
        for (index, owner) in self.owners.iter().enumerate() {
            if retired[index] {
                continue;
            }
            let status = owner
                .submit_local(IndependentCreateRequest {
                    intent: intent.clone(),
                    password: if index == 0 { password.take() } else { None },
                })
                .await;
            if status != StatusCode::ACCEPTED {
                let current = owner
                    .local_setup_state()
                    .await
                    .map_err(|_error| StatusCode::SERVICE_UNAVAILABLE)?;
                if current != (Some(intent.clone()), true) {
                    return Err(if status == StatusCode::UNAUTHORIZED {
                        StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        status
                    });
                }
            }
        }
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
        (status = 202, description = "Create accepted; check Node readiness"),
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
        (status = 409, description = "Conflicting retained setup"),
        (status = 503, description = "Setup unavailable")
    ))]
pub async fn recover(State(node): State<NodeCreate>, headers: HeaderMap) -> Response {
    no_store(
        async {
            node.authorize(&headers)?;
            let _guard = node.operation.lock().await;
            let (intent, retired) = node.retained().await?;
            Ok(match intent {
                Some(intent) => Json(CreateRecovery {
                    administrator: intent.administrator,
                    password_required: !retired[0],
                    operation_nonce: intent.operation_nonce,
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
            let _guard = node.operation.lock().await;
            let (intent, retired) = node.retained().await?;
            let intent = intent
                .filter(|intent| {
                    intent.operation_nonce == request.operation_nonce && retired != [true; 3]
                })
                .ok_or(StatusCode::CONFLICT)?;
            for owner in &node.owners {
                owner.cancel_local(&intent).await;
            }
            Ok(StatusCode::NO_CONTENT)
        }
        .await,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt, sync::Arc};

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use serde_json::{Value, json};
    use server_common::{
        IndependentCreateIntent, IndependentCreateRequest, IndependentSetupHost, SetupAdmission,
    };
    use tokio::sync::mpsc::error::TryRecvError;
    use tokio_util::sync::CancellationToken;
    use tower::ServiceExt;

    use super::NodeCreate;
    use crate::node_setup::{ADMISSION, OWNERS};

    #[tokio::test]
    async fn node_create_recovers_and_cancels_exact_partial_intent() {
        const BASE: &str = "https://node.localhost";
        const PASSWORD: &str = "test-password";
        const CREATE: &str = "/setup/node/create";
        const RECOVER: &str = "/setup/node/create/intent";
        const CANCEL: &str = "/setup/node/create/cancel";

        let root = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .expect("private setup root");
        let admission_path = root.path().join(ADMISSION);
        let client = reqwest::Client::new();
        let open_owner = |owner| {
            IndependentSetupHost::prepare_for_proxy(
                owner,
                &root.path().join(owner),
                BASE,
                BASE,
                client.clone(),
            )
            .expect("prepare owner")
            .into_create(CancellationToken::new())
            .expect("open owner control and commands")
        };
        let intent = IndependentCreateIntent {
            purpose: "create".into(),
            operation_nonce: "0123456789abcdef0123456789abcdef".into(),
            administrator: "first-admin".into(),
            node: "Retained Node".into(),
            idp: format!("{BASE}/idp"),
            issuer: format!("{BASE}/idp"),
            device: format!("{BASE}/device"),
            storage: format!("{BASE}/storage"),
        };

        {
            let _admission = SetupAdmission::open(&admission_path).expect("open Node admission");
            let [_idp, (device, _commands), _storage] = OWNERS.map(open_owner);
            assert_eq!(
                device
                    .control()
                    .submit_local(IndependentCreateRequest {
                        intent: intent.clone(),
                        password: None,
                    })
                    .await,
                StatusCode::ACCEPTED
            );
        }

        let [
            (idp, idp_commands),
            (device, device_commands),
            (storage, storage_commands),
        ] = OWNERS.map(open_owner);
        let mut receivers = [idp_commands, device_commands, storage_commands];
        let owners = [idp.control(), device.control(), storage.control()];
        for (owner, expected) in owners.iter().zip([None, Some(&intent), None]) {
            let (retained, retired) = owner
                .local_setup_state()
                .await
                .expect("read reopened state");
            assert!(!retired);
            assert!(
                retained.as_ref() == expected,
                "{} retained state",
                owner.owner()
            );
        }
        let code = SetupAdmission::protected_code(&admission_path)
            .expect("read private Node code")
            .expect("active Node code");
        let admission = SetupAdmission::open(&admission_path).expect("reopen Node admission");
        let router = NodeCreate::new(Arc::new(admission), owners, BASE.into()).router();
        let post = |path: &str, authorize: bool, body: Value| {
            let mut request = Request::post(path).header(header::CONTENT_TYPE, "application/json");
            if authorize {
                request = request.header(header::AUTHORIZATION, format!("Setup {code}"));
            }
            router.clone().oneshot(
                request
                    .body(Body::from(body.to_string()))
                    .expect("build Node request"),
            )
        };

        let response = post(RECOVER, false, Value::Null)
            .await
            .expect("unauthorized recovery response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response = post(RECOVER, true, Value::Null)
            .await
            .expect("authorized recovery response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = to_bytes(response.into_body(), 1024)
            .await
            .expect("read recovery body");
        assert_eq!(
            serde_json::from_slice::<Value>(&body).expect("recovery JSON"),
            json!({
                "administrator": intent.administrator,
                "operation_nonce": intent.operation_nonce,
                "password_required": true,
            })
        );

        let response = post(
            CREATE,
            true,
            json!({"administrator": "other-admin", "password": PASSWORD}),
        )
        .await
        .expect("conflicting administrator response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        for receiver in &mut receivers {
            assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
        }

        let response = post(
            CREATE,
            true,
            json!({"administrator": intent.administrator, "password": PASSWORD}),
        )
        .await
        .expect("exact retry response");
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let commands = receivers.map(|mut receiver| receiver.try_recv().expect("owner command"));
        for (command, password) in commands.iter().zip([Some(PASSWORD), None, None]) {
            assert!(
                command.intent == intent,
                "owner received a different intent"
            );
            assert_eq!(command.password.as_deref(), password);
        }

        let cancel = json!({"operation_nonce": intent.operation_nonce});
        let response = post(CANCEL, false, cancel.clone())
            .await
            .expect("unauthorized cancellation response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response = post(
            CANCEL,
            true,
            json!({"operation_nonce": "ffffffffffffffffffffffffffffffff"}),
        )
        .await
        .expect("wrong nonce response");
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert!(
            commands
                .iter()
                .all(|command| !command.cancellation.is_cancelled())
        );

        let response = post(CANCEL, true, cancel)
            .await
            .expect("exact cancellation response");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(
            commands
                .iter()
                .all(|command| command.cancellation.is_cancelled())
        );
    }
}
