use std::{
    fs,
    io::{self, Write},
    sync::{Arc, Mutex},
};

use api::{SetupAdmission, SetupAdmissionError};

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{MethodFilter, get, on, post},
};
use idp_server::OwnerProvisioner;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use tower_http::cors::CorsLayer;
use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};

use crate::{
    BootstrapConfig, BootstrapPurpose, CredentialBootstrap, LocalJoin, LocalProvisioner, LocalTls,
    SecurityProtocol, UnifiedRuntime,
    bootstrap_config::present,
    local_provisioning::{
        private_options, provisioning_stage, read_private, require_absent, sync_ancestry,
    },
};

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRequest {
    pub operation_nonce: String,
    pub administrator: String,
    pub node: String,
    #[schema(write_only, format = Password)]
    pub password: String,
}

#[derive(Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateIntent {
    pub operation_nonce: String,
    pub administrator: String,
    pub node: String,
}

impl CreateIntent {
    fn canonical(&self) -> Result<String, StatusCode> {
        validate_human_intent(&self.operation_nonce, &[&self.administrator, &self.node])?;
        serde_json::to_string(&(
            "create",
            ["idp", "management", "storage"],
            &self.operation_nonce,
            &self.administrator,
            &self.node,
        ))
        .map_err(|_| StatusCode::BAD_REQUEST)
    }
}

impl CreateRequest {
    fn intent(&self) -> CreateIntent {
        CreateIntent {
            operation_nonce: self.operation_nonce.clone(),
            administrator: self.administrator.clone(),
            node: self.node.clone(),
        }
    }
}

#[derive(Clone, Serialize, utoipa::ToSchema)]
pub struct CreateResult {
    pub stage: String,
    pub state: String,
    pub ready: bool,
    pub owners: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_client: Option<LoginClient>,
}

#[derive(Clone, Serialize, utoipa::ToSchema)]
pub struct LoginClient {
    pub client_id: String,
    pub scopes: Vec<String>,
}

impl CreateResult {
    fn unfinished(state: &str) -> Self {
        Self {
            stage: "installation".into(),
            state: state.into(),
            ready: false,
            owners: vec!["idp".into(), "management".into(), "storage".into()],
            login_client: None,
        }
    }
}

#[derive(Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct JoinRequest {
    pub operation_nonce: String,
    pub node: String,
    pub peer: String,
}

#[derive(Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct JoinIntent {
    pub operation_nonce: String,
    pub node: String,
    pub peer: String,
    pub issuer: String,
}

impl JoinRequest {
    fn validate(&self) -> Result<(), StatusCode> {
        validate_human_intent(&self.operation_nonce, &[&self.node])?;
        let peer = self
            .peer
            .parse::<iroh::EndpointId>()
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        if peer.to_string() != self.peer {
            return Err(StatusCode::BAD_REQUEST);
        }
        idp_server::NodeJoinRequest::validate_trusted_peer(&self.peer)
            .map_err(|_| StatusCode::BAD_REQUEST)
    }
}

impl JoinIntent {
    fn request(&self) -> JoinRequest {
        JoinRequest {
            operation_nonce: self.operation_nonce.clone(),
            node: self.node.clone(),
            peer: self.peer.clone(),
        }
    }

    fn matches(&self, request: &JoinRequest) -> bool {
        self.operation_nonce == request.operation_nonce
            && self.node == request.node
            && self.peer == request.peer
    }

    fn canonical(&self) -> Result<String, StatusCode> {
        self.request().validate()?;
        crate::bootstrap_config::validate_issuer(&self.issuer)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        serde_json::to_string(&(
            "join",
            ["idp", "management", "storage"],
            &self.operation_nonce,
            &self.node,
            &self.peer,
            &self.issuer,
        ))
        .map_err(|_| StatusCode::BAD_REQUEST)
    }
}

#[derive(Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum SetupIntent {
    Create(CreateIntent),
    Join(JoinIntent),
}

impl SetupIntent {
    fn canonical(&self) -> Result<String, StatusCode> {
        match self {
            Self::Create(intent) => intent.canonical(),
            Self::Join(intent) => intent.canonical(),
        }
    }

    fn decode(retained: &str) -> Result<Self, StatusCode> {
        let value: serde_json::Value =
            serde_json::from_str(retained).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let intent = match value.get(0).and_then(serde_json::Value::as_str) {
            Some("create") => {
                let (_, _, operation_nonce, administrator, node): (
                    String,
                    [String; 3],
                    String,
                    String,
                    String,
                ) = serde_json::from_str(retained).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
                Self::Create(CreateIntent {
                    operation_nonce,
                    administrator,
                    node,
                })
            }
            Some("join") => {
                let (_, _, operation_nonce, node, peer, issuer): (
                    String,
                    [String; 3],
                    String,
                    String,
                    String,
                    String,
                ) = serde_json::from_str(retained).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
                Self::Join(JoinIntent {
                    operation_nonce,
                    node,
                    peer,
                    issuer,
                })
            }
            _ => return Err(StatusCode::SERVICE_UNAVAILABLE),
        };
        if intent.canonical().as_deref() != Ok(retained) {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Ok(intent)
    }
}

fn validate_human_intent(nonce: &str, names: &[&str]) -> Result<(), StatusCode> {
    if nonce.len() != 32
        || !nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if names.iter().any(|name| {
        name.is_empty()
            || name.len() > 128
            || name.trim() != *name
            || name.chars().any(char::is_control)
    }) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

enum Operation {
    Create(CreateRequest),
    Join(JoinRequest),
}

struct Command {
    request: Operation,
    intent: String,
    headers: HeaderMap,
    cancellation: CancellationToken,
    reply: Option<oneshot::Sender<Result<CreateResult, StatusCode>>>,
}

pub struct CreateHost {
    host: BootstrapConfig,
    tls: Option<LocalTls>,
    shared: Arc<Shared>,
    commands: mpsc::Receiver<Command>,
    runtime: Option<UnifiedRuntime>,
    local: Option<LocalProvisioner>,
    joining: Option<LocalJoin>,
    credentials: Option<CredentialBootstrap>,
}

struct Active {
    intent: String,
    cancellation: CancellationToken,
    cancellable: bool,
}

struct Shared {
    admission: Arc<SetupAdmission>,
    commands: mpsc::Sender<Command>,
    view: Arc<RwLock<CreateResult>>,
    normal: RwLock<Option<Router>>,
    unavailable: Router,
    readiness: RwLock<Option<(SecurityProtocol, Arc<OwnerProvisioner>)>>,
    active: Mutex<Option<Active>>,
    shutdown: CancellationToken,
}

impl Shared {
    fn begin_retirement(&self, intent: &str) -> Result<(), StatusCode> {
        let mut active = self.active.lock().expect("setup cancellation lock");
        match active.as_mut() {
            Some(active)
                if active.intent == intent
                    && active.cancellable
                    && !active.cancellation.is_cancelled()
                    && !self.shutdown.is_cancelled() =>
            {
                active.cancellable = false;
                Ok(())
            }
            _ => Err(StatusCode::CONFLICT),
        }
    }
}

impl CreateHost {
    pub(crate) fn new(
        host: BootstrapConfig,
        tls: impl Into<Option<LocalTls>>,
        admission: Arc<SetupAdmission>,
        shutdown: CancellationToken,
    ) -> Self {
        let (sender, commands) = mpsc::channel(1);
        let state = match host.purpose() {
            BootstrapPurpose::AwaitingChoice => "awaiting_choice",
            BootstrapPurpose::Create => "creating",
            BootstrapPurpose::Join => "joining",
        };
        let view = Arc::new(RwLock::new(CreateResult::unfinished(state)));
        let unavailable = owner_routes(&host.config()).with_state(view.clone());
        Self {
            host,
            tls: tls.into(),
            commands,
            shared: Arc::new(Shared {
                admission,
                commands: sender,
                view,
                normal: RwLock::new(None),
                unavailable,
                readiness: RwLock::new(None),
                active: Mutex::new(None),
                shutdown,
            }),
            runtime: None,
            local: None,
            joining: None,
            credentials: None,
        }
    }

    pub fn router(&self) -> io::Result<Router> {
        let value = self.host.config().idp.ui_public_uri;
        let origin = if self.tls.is_some() {
            api::setup_origin(&value)?
        } else {
            crate::setup_host::allowed_origin(&value)?
        };
        self.router_for_origin(origin)
    }

    /// Host-selected UI origin, including a bundled native WebView's exact origin.
    /// This is not request-controlled and does not replace setup admission.
    pub fn router_for_origin(&self, origin: axum::http::HeaderValue) -> io::Result<Router> {
        if origin != "tauri://localhost" && origin != "http://tauri.localhost" {
            let value = origin.to_str().map_err(io::Error::other)?;
            if self.tls.is_some() {
                api::setup_origin(value)?;
            } else {
                crate::setup_host::allowed_origin(value)?;
            }
        }
        let config = self.host.config();
        let owners = idp_server::public_metadata_router(&config.idp.api_public_uri, "/idp")
            .merge(management_server::public_metadata_router(
                &config.management.api_public_uri,
                "/management",
            ))
            .merge(storage_server::public_metadata_router(
                &config.storage.api_public_uri,
                "/storage",
            ))
            .with_state(());
        let setup = Router::new()
            .merge(owners)
            .route("/idp/setup/status", get(status))
            .route("/idp/setup/create", post(create))
            .route("/idp/setup/join", post(join))
            .route("/idp/setup/cancel", post(cancel))
            .route("/idp/setup/admission", post(admission))
            .route("/idp/setup/intent", post(intent))
            .route("/idp/setup/openapi.json", get(setup_openapi_json))
            .fallback(normal)
            .with_state(self.shared.clone())
            .layer(
                CorsLayer::new()
                    .allow_origin(origin.clone())
                    .allow_methods([
                        axum::http::Method::GET,
                        axum::http::Method::HEAD,
                        axum::http::Method::POST,
                        axum::http::Method::PUT,
                        axum::http::Method::PATCH,
                        axum::http::Method::DELETE,
                        axum::http::Method::OPTIONS,
                    ])
                    .allow_headers([
                        axum::http::header::AUTHORIZATION,
                        axum::http::header::CONTENT_TYPE,
                    ])
                    .allow_private_network(true),
            )
            .layer(middleware::from_fn_with_state(
                origin,
                api::setup_origin_guard,
            ));
        Ok(setup)
    }

    pub(crate) async fn initialize(&mut self) {
        let reopen = Box::pin(self.reopen()).await;
        if let Err(error) = reopen {
            if let Err(cleanup) = self.unpublish().await {
                log::info!("Node reopen cleanup failed kind={:?}", cleanup.kind());
            }
            log::info!("Node reopen failed kind={:?}", error.kind());
            *self.shared.view.write().await = CreateResult::unfinished("unavailable");
        } else if self.host.purpose() != BootstrapPurpose::AwaitingChoice {
            let mut view = self.shared.view.write().await;
            if !view.ready {
                *view = CreateResult::unfinished("unfinished");
            }
        }
    }

    pub async fn run(mut self) -> io::Result<()> {
        let shutdown = self.shared.shutdown.clone();
        self.initialize().await;
        loop {
            let mut command = tokio::select! {
                biased;
                () = shutdown.cancelled() => break,
                command = self.commands.recv() => match command { Some(command) => command, None => break },
            };
            let result = Box::pin(self.execute(&mut command)).await;
            if let Some(reply) = command.reply.take() {
                let _ = reply.send(result);
            }
            *self.shared.active.lock().expect("Create cancellation lock") = None;
        }
        self.unpublish().await
    }

    async fn reopen(&mut self) -> io::Result<()> {
        let started = std::time::Instant::now();
        if self.host.purpose() == BootstrapPurpose::Join {
            return Box::pin(self.reopen_join()).await;
        }
        if self.host.purpose() != BootstrapPurpose::Create {
            return Ok(());
        }
        let config = self.host.config();
        if !present(&config.credential_bootstrap_directory())? {
            return Ok(());
        }
        let owner_root = config.local_provisioning_directory();
        self.credentials = Some(CredentialBootstrap::open(config)?);
        if !present(&owner_root)? {
            return Ok(());
        }
        let config = self
            .credentials
            .as_ref()
            .expect("retained credential references")
            .load()?;
        let local = LocalProvisioner::open(config)?;
        log::info!(
            "Node reopen stage=credentials elapsed_ms={}",
            started.elapsed().as_millis()
        );
        let started = std::time::Instant::now();
        let admission = self.shared.admission.clone();
        let retired = tokio::task::spawn_blocking(move || admission.is_retired())
            .await
            .map_err(io::Error::other)?
            .map_err(io::Error::other)?;
        if local.is_complete()? && retired {
            let runtime = local.reopen_hosted_runtime(self.tls.is_some()).await?;
            log::info!(
                "Node reopen stage=runtime elapsed_ms={}",
                started.elapsed().as_millis()
            );
            self.local = Some(local);
            self.publish(runtime).await?;
            *self.shared.view.write().await = self.ready_result()?;
            log::info!(
                "Node reopen ready elapsed_ms={}",
                started.elapsed().as_millis()
            );
        } else {
            self.local = Some(local);
        }
        Ok(())
    }

    async fn execute(&mut self, command: &mut Command) -> Result<CreateResult, StatusCode> {
        if command.cancellation.is_cancelled() {
            return Err(StatusCode::CONFLICT);
        }
        if self.runtime.is_some() && self.shared.view.read().await.ready {
            return self.completed_retry(command).await;
        }
        if let Operation::Join(request) = &command.request {
            let admission = self.shared.admission.clone();
            let headers = command.headers.clone();
            let retained = tokio::task::spawn_blocking(move || admission.bound_intent(&headers))
                .await
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
                .map_err(admission_status)?;
            let intent = if let Some(retained) = retained {
                match SetupIntent::decode(&retained)? {
                    SetupIntent::Join(intent) if intent.matches(request) => intent,
                    _ => return Err(StatusCode::CONFLICT),
                }
            } else {
                let issuer = crate::issuer_discovery::discover_with_cancellation(
                    &request.peer,
                    &command.cancellation,
                )
                .await
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
                JoinIntent {
                    operation_nonce: request.operation_nonce.clone(),
                    node: request.node.clone(),
                    peer: request.peer.clone(),
                    issuer,
                }
            };
            command.intent = intent.canonical()?;
        }
        if command.cancellation.is_cancelled() {
            return Err(StatusCode::CONFLICT);
        }
        let admission = self.shared.admission.clone();
        let headers = command.headers.clone();
        let intent = command.intent.clone();
        tokio::task::spawn_blocking(move || admission.bind(&headers, &intent))
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .map_err(admission_status)?;
        if matches!(&command.request, Operation::Join(_)) {
            self.retain_join_intent(&command.intent)
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        }
        *self.shared.active.lock().expect("Create cancellation lock") = Some(Active {
            intent: command.intent.clone(),
            cancellation: command.cancellation.clone(),
            cancellable: true,
        });
        let accepted = CreateResult::unfinished(match &command.request {
            Operation::Create(_) => "creating",
            Operation::Join(_) => "joining",
        });
        *self.shared.view.write().await = accepted.clone();
        if let Some(reply) = command.reply.take() {
            let _ = reply.send(Ok(accepted));
        }
        let started = std::time::Instant::now();
        let result = match &command.request {
            Operation::Create(request) => Box::pin(self.provision(command, request)).await,
            Operation::Join(request) => Box::pin(self.provision_join(command, request)).await,
        };
        log::info!(
            "Node setup elapsed_ms={} succeeded={}",
            started.elapsed().as_millis(),
            result.is_ok()
        );
        if result.is_err() {
            if let Err(error) = self.unpublish().await {
                log::info!("Node setup cleanup failed kind={:?}", error.kind());
                *self.shared.view.write().await = CreateResult::unfinished("unavailable");
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
            *self.shared.view.write().await = CreateResult::unfinished("unfinished");
        }
        result
    }

    async fn provision(
        &mut self,
        command: &Command,
        request: &CreateRequest,
    ) -> Result<CreateResult, StatusCode> {
        self.host
            .select_create()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        self.open_credentials()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let credentials = self.credentials.take().expect("retained credentials guard");
        let administrator = request.administrator.clone();
        let node = request.node.clone();
        let password = request.password.clone();
        let local = self.local.take();
        let prepared = tokio::task::spawn_blocking(move || {
            let result = (|| {
                let config = credentials.load()?;
                let local = match local {
                    Some(local) => local,
                    None if present(&config.local_provisioning_directory())? => {
                        LocalProvisioner::open(config)?
                    }
                    None => LocalProvisioner::create_with_password(
                        config,
                        &administrator,
                        &node,
                        &password,
                    )?,
                };
                local.validate_retry(&administrator, &node, &password)?;
                Ok::<_, io::Error>(local)
            })();
            (credentials, result)
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        self.credentials = Some(prepared.0);
        self.local = Some(prepared.1.map_err(|_| StatusCode::CONFLICT)?);
        let local = self.local.as_mut().expect("retained owner journal");
        let runtime = Box::pin(local.prepare_hosted_runtime(
            &request.password,
            self.tls.is_some(),
            &command.cancellation,
        ))
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if let Err(error) = self.publish(runtime).await {
            return Err(if error.kind() == io::ErrorKind::Interrupted {
                StatusCode::CONFLICT
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            });
        }
        if command.cancellation.is_cancelled() {
            return Err(StatusCode::CONFLICT);
        }
        self.local
            .as_ref()
            .expect("verified owner journal")
            .mark_complete()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        self.retire(command).await
    }

    fn retain_join_intent(&self, canonical: &str) -> io::Result<()> {
        let path = self.host.directory().join("join-admission.json");
        let pending = self.host.directory().join("join-admission.pending");
        require_absent(&pending)?;
        if present(&path)? {
            if read_private(&path)? != canonical.as_bytes() {
                return Err(io::Error::other(
                    "Join admission differs from retained host intent",
                ));
            }
            return self.initialize_join_history();
        }
        if self.host.purpose() != BootstrapPurpose::AwaitingChoice {
            return Err(io::Error::other(
                "selected Join is missing its retained admission intent",
            ));
        }
        let mut file = private_options(true).open(&pending)?;
        file.write_all(canonical.as_bytes())?;
        file.sync_all()?;
        fs::rename(pending, &path)?;
        sync_ancestry(self.host.directory())?;
        if read_private(&path)? != canonical.as_bytes() {
            return Err(io::Error::other("Join admission durable readback mismatch"));
        }
        self.initialize_join_history()
    }

    fn initialize_join_history(&self) -> io::Result<()> {
        for stage in ["credentials", "owner"] {
            let path = self
                .host
                .directory()
                .join(format!("join-{stage}.attempted"));
            if !present(&path)? {
                if self.host.purpose() != BootstrapPurpose::AwaitingChoice {
                    return Err(io::Error::other(
                        "selected Join is missing production history",
                    ));
                }
                let mut file = private_options(true).open(&path)?;
                file.write_all(b"unattempted\n")?;
                file.sync_all()?;
                sync_ancestry(self.host.directory())?;
            }
            Self::join_attempted(self.host.directory(), stage)?;
        }
        Ok(())
    }

    fn join_attempted(directory: &std::path::Path, stage: &str) -> io::Result<bool> {
        require_absent(&directory.join(format!("join-{stage}.pending")))?;
        match read_private(&directory.join(format!("join-{stage}.attempted")))?.as_slice() {
            b"unattempted\n" => Ok(false),
            b"attempted\n" => Ok(true),
            _ => Err(io::Error::other("invalid retained Join production history")),
        }
    }

    fn begin_join_attempt(
        directory: &std::path::Path,
        stage: &str,
        root: &std::path::Path,
    ) -> io::Result<bool> {
        if Self::join_attempted(directory, stage)? {
            return Ok(false);
        }
        require_absent(root)?;
        let path = directory.join(format!("join-{stage}.attempted"));
        let pending = directory.join(format!("join-{stage}.pending"));
        let mut file = private_options(true).open(&pending)?;
        file.write_all(b"attempted\n")?;
        file.sync_all()?;
        fs::rename(pending, path)?;
        sync_ancestry(directory)?;
        if !Self::join_attempted(directory, stage)? {
            return Err(io::Error::other(
                "Join production history readback mismatch",
            ));
        }
        Ok(true)
    }

    fn retained_join_intent(&self) -> io::Result<JoinIntent> {
        require_absent(&self.host.directory().join("join-admission.pending"))?;
        let bytes = read_private(&self.host.directory().join("join-admission.json"))?;
        let canonical = std::str::from_utf8(&bytes).map_err(io::Error::other)?;
        match SetupIntent::decode(canonical)
            .map_err(|_| io::Error::other("invalid retained Join admission"))?
        {
            SetupIntent::Join(intent)
                if Some(intent.peer.as_str()) == self.host.trusted_peer()
                    && intent.issuer == self.host.config().idp.oauth2.issuer =>
            {
                Ok(intent)
            }
            _ => Err(io::Error::other(
                "retained Join admission conflicts with host identity",
            )),
        }
    }

    async fn reopen_join(&mut self) -> io::Result<()> {
        #[cfg(test)]
        eprintln!("Node Join reopen stage=intent");
        let intent = self.retained_join_intent()?;
        let canonical = intent
            .canonical()
            .map_err(|_| io::Error::other("invalid retained Join admission"))?;
        let admission = self.shared.admission.clone();
        let retired = tokio::task::spawn_blocking(move || {
            let retired = admission.is_retired().map_err(io::Error::other)?;
            if retired
                && !admission
                    .retired_matches(&canonical)
                    .map_err(io::Error::other)?
            {
                return Err(io::Error::other(
                    "retired admission differs from completed Join intent",
                ));
            }
            Ok::<_, io::Error>(retired)
        })
        .await
        .map_err(io::Error::other)??;
        #[cfg(test)]
        eprintln!("Node Join reopen stage=history");
        let config = self.host.config();
        let credentials_attempted = Self::join_attempted(self.host.directory(), "credentials")?;
        let owner_attempted = Self::join_attempted(self.host.directory(), "owner")?;
        if owner_attempted && !credentials_attempted
            || retired && !(credentials_attempted && owner_attempted)
        {
            return Err(io::Error::other(
                "retired or conflicting Join production history",
            ));
        }
        if !credentials_attempted {
            require_absent(&config.credential_bootstrap_directory())?;
            require_absent(&config.local_provisioning_directory())?;
            return Ok(());
        }
        if !present(&config.credential_bootstrap_directory())? {
            return Err(io::Error::other(
                "attempted Join is missing retained credentials",
            ));
        }
        let owner_root = config.local_provisioning_directory();
        #[cfg(test)]
        eprintln!("Node Join reopen stage=credentials");
        self.credentials = Some(CredentialBootstrap::open(config)?);
        if !owner_attempted {
            require_absent(&owner_root)?;
            return Ok(());
        }
        let config = self
            .credentials
            .as_ref()
            .expect("retained credential references")
            .load()?;
        if !present(&config.local_provisioning_directory())? {
            return Err(io::Error::other(
                "attempted Join is missing retained owner state",
            ));
        }
        #[cfg(test)]
        eprintln!("Node Join reopen stage=owner");
        let joining = LocalJoin::open(config)?;
        let complete = joining.is_complete()?;
        joining.validate_retry(&intent.peer, &intent.node)?;
        if Some(joining.trusted_peer()) != self.host.trusted_peer() {
            return Err(io::Error::other(
                "retained Join peer conflicts with host selection",
            ));
        }
        self.joining = Some(joining);
        if complete && retired {
            #[cfg(test)]
            eprintln!("Node Join reopen stage=runtime");
            let runtime = self
                .joining
                .as_ref()
                .expect("retained Join")
                .reopen_hosted_runtime(self.tls.is_some())
                .await?;
            #[cfg(test)]
            eprintln!("Node Join reopen stage=publish");
            self.publish(runtime).await?;
            *self.shared.view.write().await = self.ready_result()?;
        } else if retired {
            return Err(io::Error::other(
                "retired admission requires completed Join",
            ));
        }
        Ok(())
    }

    fn open_credentials(&mut self) -> io::Result<()> {
        if self.credentials.is_some()
            && self.host.purpose() == BootstrapPurpose::Join
            && !Self::join_attempted(self.host.directory(), "credentials")?
        {
            return Err(io::Error::other(
                "retained Join is missing credential production history",
            ));
        }
        if self.credentials.is_none() {
            let config = self.host.config();
            self.credentials = Some(if self.host.purpose() == BootstrapPurpose::Join {
                if Self::begin_join_attempt(
                    self.host.directory(),
                    "credentials",
                    &config.credential_bootstrap_directory(),
                )? {
                    match &self.tls {
                        Some(tls) => CredentialBootstrap::adopt_host_tls(config, tls)?,
                        None => CredentialBootstrap::create(config)?,
                    }
                } else {
                    CredentialBootstrap::open(config)?
                }
            } else if present(&config.credential_bootstrap_directory())? {
                CredentialBootstrap::open(config)?
            } else {
                match &self.tls {
                    Some(tls) => CredentialBootstrap::adopt_host_tls(config, tls)?,
                    None => CredentialBootstrap::create(config)?,
                }
            });
        }
        Ok(())
    }

    async fn provision_join(
        &mut self,
        command: &Command,
        request: &JoinRequest,
    ) -> Result<CreateResult, StatusCode> {
        let SetupIntent::Join(intent) = SetupIntent::decode(&command.intent)? else {
            return Err(StatusCode::CONFLICT);
        };
        self.host
            .select_join(&intent.peer, &intent.issuer)
            .map_err(|_| StatusCode::CONFLICT)?;
        self.open_credentials()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let directory = self.host.directory().to_path_buf();
        let credentials = self.credentials.take().expect("retained credential guard");
        let joining = self.joining.take();
        let request = request.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            let result = (|| {
                let config = credentials.load()?;
                let joining = match joining {
                    Some(joining) => {
                        if !Self::join_attempted(&directory, "owner")? {
                            return Err(io::Error::other(
                                "retained Join is missing owner production history",
                            ));
                        }
                        joining
                    }
                    None if Self::begin_join_attempt(
                        &directory,
                        "owner",
                        &config.local_provisioning_directory(),
                    )? =>
                    {
                        LocalJoin::create(config, &request.peer, &request.node)?
                    }
                    None => LocalJoin::open(config)?,
                };
                joining.validate_retry(&request.peer, &request.node)?;
                Ok::<_, io::Error>(joining)
            })();
            (credentials, result)
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        self.credentials = Some(prepared.0);
        self.joining = Some(prepared.1.map_err(|_| StatusCode::CONFLICT)?);
        let joining = self.joining.as_mut().expect("retained Join");
        let runtime = provisioning_stage(|| {
            joining.prepare_hosted_runtime(self.tls.is_some(), &command.cancellation)
        })
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        #[cfg(test)]
        let publish_started = std::time::Instant::now();
        self.publish(runtime)
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        #[cfg(test)]
        eprintln!(
            "Node Join stage=publish elapsed_ms={}",
            publish_started.elapsed().as_millis()
        );
        #[cfg(test)]
        let completion_started = std::time::Instant::now();
        self.joining
            .as_mut()
            .expect("verified Join")
            .complete_hosted(
                self.runtime.as_ref().expect("published runtime"),
                &command.cancellation,
            )
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        #[cfg(test)]
        eprintln!(
            "Node Join stage=completion elapsed_ms={}",
            completion_started.elapsed().as_millis()
        );
        self.retire(command).await
    }

    async fn retire(&self, command: &Command) -> Result<CreateResult, StatusCode> {
        self.shared.begin_retirement(&command.intent)?;
        let admission = self.shared.admission.clone();
        let headers = command.headers.clone();
        let intent = command.intent.clone();
        tokio::task::spawn_blocking(move || admission.retire(&headers, &intent))
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .map_err(admission_status)?;
        let result = self
            .ready_result()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        *self.shared.view.write().await = result.clone();
        Ok(result)
    }

    async fn publish(&mut self, mut runtime: UnifiedRuntime) -> io::Result<()> {
        #[cfg(test)]
        runtime.trace_bootstrap_http_stages();
        if let Err(error) = runtime.start_background_tasks().await {
            runtime.shutdown().await?;
            return Err(error);
        }
        *self.shared.readiness.write().await = Some(runtime.readiness());
        *self.shared.normal.write().await = Some(runtime.router());
        self.runtime = Some(runtime);
        Ok(())
    }

    async fn unpublish(&mut self) -> io::Result<()> {
        *self.shared.readiness.write().await = None;
        *self.shared.normal.write().await = None;
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown().await?;
        }
        Ok(())
    }

    fn ready_result(&self) -> io::Result<CreateResult> {
        let login = match self.joining.as_ref() {
            Some(joining) => joining.login_client(),
            None => self
                .local
                .as_ref()
                .expect("ready owner journal")
                .login_client()?,
        };
        let mut result = CreateResult::unfinished("ready");
        result.ready = true;
        result.login_client = Some(LoginClient {
            client_id: login
                .client_id
                .ok_or_else(|| io::Error::other("missing public login Client"))?,
            scopes: login.allowed_scopes,
        });
        Ok(result)
    }

    async fn completed_retry(&mut self, command: &Command) -> Result<CreateResult, StatusCode> {
        let Operation::Create(request) = &command.request else {
            return Err(StatusCode::UNAUTHORIZED);
        };
        let admission = self.shared.admission.clone();
        let intent = command.intent.clone();
        if !tokio::task::spawn_blocking(move || admission.retired_matches(&intent))
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .map_err(admission_status)?
        {
            return Err(StatusCode::CONFLICT);
        }
        let result = self
            .ready_result()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let client_id = &result
            .login_client
            .as_ref()
            .expect("ready public login Client")
            .client_id;
        let http = LocalTls::verification_client().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let response = http
            .post(format!(
                "{}://{}/idp/oauth2/token",
                if self.tls.is_some() { "https" } else { "http" },
                self.host.config().listen_addr
            ))
            .form(&[
                ("grant_type", "password"),
                ("client_id", client_id.as_str()),
                ("username", request.administrator.as_str()),
                ("password", request.password.as_str()),
                ("scope", "openid"),
            ])
            .send()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let token: model::contract::TokenResponse = response
            .json()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        if token.access_token.0.is_empty() {
            return Err(StatusCode::UNAUTHORIZED);
        }
        if !self
            .runtime
            .as_ref()
            .expect("completed runtime")
            .node_public_ready()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Ok(result)
    }
}

fn admission_status(error: SetupAdmissionError) -> StatusCode {
    match error {
        SetupAdmissionError::Denied => StatusCode::UNAUTHORIZED,
        SetupAdmissionError::Conflict => StatusCode::CONFLICT,
        SetupAdmissionError::InvalidIntent => StatusCode::BAD_REQUEST,
        SetupAdmissionError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    }
}

#[utoipa::path(
    post, path = "/idp/setup/admission",
    description = "Check the host-local setup capability only. A successful check neither binds intent nor authorizes a later request; send Authorization on every setup mutation.",
    responses(
        (status = 204, description = "Setup capability currently valid; no body"),
        (status = 401, description = "Missing, invalid or retired setup capability"),
        (status = 403, description = "Disallowed or multiple Origin headers"),
        (status = 503, description = "Protected admission state unavailable")
    ),
    security(("setup_authorization" = []))
)]
async fn admission(State(shared): State<Arc<Shared>>, headers: HeaderMap) -> StatusCode {
    let admission = shared.admission.clone();
    match tokio::task::spawn_blocking(move || admission.check(&headers)).await {
        Ok(Ok(())) => StatusCode::NO_CONTENT,
        Ok(Err(error)) => admission_status(error),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

#[utoipa::path(
    post, path = "/idp/setup/intent",
    description = "Read the exact retained nonsecret unified Create or Join intent using the current setup capability. Join includes the authenticated peer and discovered issuer. No request body; does not resume mutation or authorize another request. Retired capabilities deny.",
    responses(
        (status = 200, body = SetupIntent, description = "Validated canonical retained intent for this host's owner scope"),
        (status = 204, description = "No intent bound; no body"),
        (status = 401, description = "Missing, invalid or retired setup capability"),
        (status = 403, description = "Disallowed or multiple Origin headers"),
        (status = 503, description = "Protected state unavailable or retained intent invalid; no body")
    ),
    security(("setup_authorization" = []))
)]
async fn intent(State(shared): State<Arc<Shared>>, headers: HeaderMap) -> Response {
    let admission = shared.admission.clone();
    let retained = match tokio::task::spawn_blocking(move || admission.bound_intent(&headers)).await
    {
        Ok(Ok(Some(retained))) => retained,
        Ok(Ok(None)) => return StatusCode::NO_CONTENT.into_response(),
        Ok(Err(error)) => return admission_status(error).into_response(),
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    match SetupIntent::decode(&retained) {
        Ok(intent) => Json(intent).into_response(),
        Err(status) => status.into_response(),
    }
}

#[utoipa::path(
    get, path = "/idp/setup/status",
    description = "Public nonsecret setup observation, not mutation authority. Poll after 202. Completed status re-evaluates existing local owner readiness on every request; learned revocation or a failed check returns 503 with ready:false and no login Client. Normal protected APIs retain their independent authentication/authorization. No password, token or setup code is returned.",
    responses(
        (status = 200, body = CreateResult, description = "Current setup state; awaiting_choice, creating, joining and unfinished are not completion"),
        (status = 403, description = "Disallowed or multiple Origin headers"),
        (status = 503, body = CreateResult, description = "Setup unavailable; ready:false")
    ),
    security()
)]
async fn status(State(shared): State<Arc<Shared>>) -> Response {
    let admission = shared.admission.clone();
    if !matches!(
        tokio::task::spawn_blocking(move || admission.health()).await,
        Ok(Ok(()))
    ) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(CreateResult::unfinished("unavailable")),
        )
            .into_response();
    }
    let result = shared.view.read().await.clone();
    if result.ready {
        let readiness = shared.readiness.read().await.clone();
        let current = match readiness {
            Some((protocol, owner)) => matches!(
                protocol.local_node_public_ready(owner.as_ref()).await,
                Ok(true)
            ),
            None => false,
        };
        if !current || shared.shutdown.is_cancelled() {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(CreateResult::unfinished("unavailable")),
            )
                .into_response();
        }
    }
    (
        if result.state == "unavailable" {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::OK
        },
        Json(result),
    )
        .into_response()
}

#[utoipa::path(
    post, path = "/idp/setup/create", request_body = CreateRequest,
    description = "Before completion, Authorization: Setup <code> is required for every request, including exact retries. Durable exact intent binding precedes 202 and owner mutation. Acceptance is not completion: poll /idp/setup/status. Request disconnect does not cancel host-owned work; use explicit cancel. After completion, only an exact retained retry authenticated through the normal password grant using the supplied administrator/password can return 200; no setup header or Bearer token is required for that retry.",
    responses(
        (status = 202, body = CreateResult, description = "Durably admitted or already active exact Create; ready:false; observe completion through setup status"),
        (status = 200, body = CreateResult, description = "Exact completed retry authenticated with normal password grant; ready:true"),
        (status = 400, description = "Invalid human intent/password (empty body), or malformed/unreadable JSON (text/plain rejection)"),
        (status = 401, description = "Setup capability or normal completed authentication denied"),
        (status = 403, description = "Disallowed or multiple Origin headers"),
        (status = 409, description = "Conflicting retained/active intent, occupied command queue or cancellation before acceptance"),
        (status = 413, description = "JSON request exceeds body limit", body = String, content_type = "text/plain"),
        (status = 415, description = "Missing JSON content type", body = String, content_type = "text/plain"),
        (status = 422, description = "JSON does not match CreateRequest", body = String, content_type = "text/plain"),
        (status = 503, description = "Admission, coordinator or completed readiness/authentication unavailable; no body. Accepted provisioning failures are observed through status, not this response")
    ),
    security((), ("setup_authorization" = []))
)]
async fn create(
    State(shared): State<Arc<Shared>>,
    headers: HeaderMap,
    Json(request): Json<CreateRequest>,
) -> Response {
    let intent = match request.intent().canonical() {
        Ok(intent) => intent,
        Err(status) => return status.into_response(),
    };
    if request.password.is_empty()
        || request.password.len() > 1024
        || request.password.chars().any(char::is_control)
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    submit(shared, headers, Operation::Create(request), intent).await
}

#[utoipa::path(
    post, path = "/idp/setup/join", request_body = JoinRequest,
    description = "Requires Authorization: Setup <code> on every request. Authenticates issuer discovery against the pinned peer before durable canonical Join admission and 202. No issuer or credentials are accepted from the caller. Accepted work belongs to the host, not the HTTP request; poll status and use authorized exact cancellation. Exact retries retain peer/issuer and identity, including complete-owner/unretired-admission finalization. Retired admission denies; normal User authorization remains separate on source approval routes.",
    responses(
        (status = 202, body = CreateResult, description = "Durably admitted or already active exact Join; joining is not readiness"),
        (status = 400, description = "Invalid nonce, Node name or canonical pinned peer"),
        (status = 401, description = "Missing, invalid or retired setup capability"),
        (status = 403, description = "Disallowed Origin"),
        (status = 409, description = "Conflicting intent or occupied coordinator"),
        (status = 413, description = "Request exceeds body limit", body = String, content_type = "text/plain"),
        (status = 415, description = "Missing JSON content type", body = String, content_type = "text/plain"),
        (status = 422, description = "JSON does not match JoinRequest", body = String, content_type = "text/plain"),
        (status = 503, description = "Discovery, admission or coordinator unavailable; accepted owner failures appear in status")
    ),
    security(("setup_authorization" = []))
)]
async fn join(
    State(shared): State<Arc<Shared>>,
    headers: HeaderMap,
    Json(request): Json<JoinRequest>,
) -> Response {
    if let Err(status) = request.validate() {
        return status.into_response();
    }
    submit(shared, headers, Operation::Join(request), String::new()).await
}

async fn submit(
    shared: Arc<Shared>,
    headers: HeaderMap,
    request: Operation,
    intent: String,
) -> Response {
    let joining = matches!(&request, Operation::Join(_));
    if joining || !shared.view.read().await.ready {
        let admission = shared.admission.clone();
        let supplied = headers.clone();
        match tokio::task::spawn_blocking(move || admission.check(&supplied)).await {
            Ok(Ok(())) => (),
            Ok(Err(error)) => return error.into_response(),
            Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        }
    }
    let response = {
        let active = shared.active.lock().expect("Create cancellation lock");
        if let Some(active) = active.as_ref() {
            if !active.cancellable || active.cancellation.is_cancelled() {
                return StatusCode::CONFLICT.into_response();
            }
            let matches = match &request {
                Operation::Create(_) => active.intent == intent,
                Operation::Join(request) => {
                    matches!(SetupIntent::decode(&active.intent), Ok(SetupIntent::Join(intent)) if intent.matches(request))
                }
            };
            return if matches {
                (
                    StatusCode::ACCEPTED,
                    Json(CreateResult::unfinished(if joining {
                        "joining"
                    } else {
                        "creating"
                    })),
                )
                    .into_response()
            } else {
                StatusCode::CONFLICT.into_response()
            };
        }
        let cancellation = shared.shutdown.child_token();
        let (reply, response) = oneshot::channel();
        let command = Command {
            request,
            intent,
            headers,
            cancellation,
            reply: Some(reply),
        };
        if shared.commands.try_send(command).is_err() {
            return StatusCode::CONFLICT.into_response();
        }
        response
    };
    match response.await {
        Ok(Ok(result)) => (
            if result.ready {
                StatusCode::OK
            } else {
                StatusCode::ACCEPTED
            },
            Json(result),
        )
            .into_response(),
        Ok(Err(status)) => status.into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[utoipa::path(
    post, path = "/idp/setup/cancel", request_body = SetupIntent,
    description = "Requires the setup capability and exact active nonsecret Create or Join intent (including discovered Join issuer, recoverable through /idp/setup/intent). Cancellation and entry into retirement serialize on the same active-operation lock: cancellation cannot acknowledge 204 after retirement starts (409 while the capability remains valid; retired capabilities deny). A 204 is not proof cleanup has finished; observe status. Disconnect alone never cancels accepted work.",
    responses(
        (status = 204, description = "Exact active cancellation token signaled; no body; cleanup may still be running"),
        (status = 400, description = "Invalid intent (empty body), or malformed/unreadable JSON (text/plain rejection)"),
        (status = 401, description = "Missing, invalid or retired setup capability"),
        (status = 403, description = "Disallowed or multiple Origin headers"),
        (status = 409, description = "No active cancellable work, mismatching intent or retirement already started"),
        (status = 413, description = "JSON request exceeds body limit", body = String, content_type = "text/plain"),
        (status = 415, description = "Missing JSON content type", body = String, content_type = "text/plain"),
        (status = 422, description = "JSON does not match a Create or Join intent", body = String, content_type = "text/plain"),
        (status = 503, description = "Protected admission state unavailable")
    ),
    security(("setup_authorization" = []))
)]
async fn cancel(
    State(shared): State<Arc<Shared>>,
    headers: HeaderMap,
    Json(intent): Json<SetupIntent>,
) -> StatusCode {
    let intent = match intent.canonical() {
        Ok(intent) => intent,
        Err(status) => return status,
    };
    let admission = shared.admission.clone();
    match tokio::task::spawn_blocking(move || admission.check(&headers)).await {
        Ok(Ok(())) => (),
        Ok(Err(error)) => return admission_status(error),
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE,
    }
    let active = shared.active.lock().expect("Create cancellation lock");
    match active.as_ref() {
        Some(active) if active.intent == intent && active.cancellable => {
            active.cancellation.cancel();
            StatusCode::NO_CONTENT
        }
        _ => StatusCode::CONFLICT,
    }
}

async fn normal(State(shared): State<Arc<Shared>>, request: Request) -> Response {
    let router = shared
        .normal
        .read()
        .await
        .clone()
        .unwrap_or_else(|| shared.unavailable.clone());
    router
        .oneshot(request)
        .await
        .expect("infallible owner router")
}

async fn unavailable(State(view): State<Arc<RwLock<CreateResult>>>) -> Response {
    let state = view.read().await;
    let error = if state.state == "awaiting_choice" {
        "setup_required"
    } else {
        "node_not_ready"
    };
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error": error})),
    )
        .into_response()
}

fn owner_routes(config: &crate::UnifiedConfig) -> Router<Arc<RwLock<CreateResult>>> {
    let mut router = Router::new();
    for (prefix, document) in [
        (
            "/idp",
            idp_server::openapi_document(&config.idp.api_public_uri, ""),
        ),
        (
            "/management",
            management_server::openapi_document(&config.management.api_public_uri, ""),
        ),
        (
            "/storage",
            storage_server::openapi_document(&config.storage.api_public_uri, "/storage"),
        ),
    ] {
        for (path, item) in document.paths.paths {
            if matches!(path.as_str(), "/version" | "/openapi.json") {
                continue;
            }
            for (present, method) in [
                (item.get.is_some(), MethodFilter::GET),
                (item.put.is_some(), MethodFilter::PUT),
                (item.post.is_some(), MethodFilter::POST),
                (item.delete.is_some(), MethodFilter::DELETE),
                (item.options.is_some(), MethodFilter::OPTIONS),
                (item.head.is_some(), MethodFilter::HEAD),
                (item.patch.is_some(), MethodFilter::PATCH),
                (item.trace.is_some(), MethodFilter::TRACE),
            ] {
                if present {
                    let path = if path == "/" {
                        prefix.to_owned()
                    } else {
                        format!("{prefix}{path}")
                    };
                    router = router.route(&path, on(method, unavailable));
                }
            }
        }
    }
    router
        .route(
            "/management/node-joins/acceptance",
            get(unavailable).put(unavailable),
        )
        .route("/management/node-joins/pending", get(unavailable))
        .route("/management/node-joins/{id}/approve", post(unavailable))
        .route("/management/node-joins/{id}/reject", post(unavailable))
}

#[derive(OpenApi)]
#[openapi(
    paths(create, join, status, cancel, admission, intent, setup_openapi_json),
    servers((url = "https://localhost", description = "Set the intended verified HTTPS host origin explicitly; operation paths include the IdP prefix")),
    components(schemas(CreateRequest, CreateIntent, JoinRequest, JoinIntent, SetupIntent, CreateResult, LoginClient)),
    modifiers(&SetupSecurity)
)]
struct CreateDoc;

struct SetupSecurity;

impl Modify for SetupSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        openapi
            .components
            .as_mut()
            .expect("CreateDoc declares schema components")
            .add_security_scheme(
                "setup_authorization",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                    "Authorization",
                    "Send the complete value Setup <code> over verified HTTPS, not an OAuth Bearer token. Retrieve the host-generated code only through protected host/deployment management. Keep it transient; never put it in URLs, logs, cookies or browser storage. Required for every Join/retry and before completion for every Create/retry, admission check, retained intent lookup and cancel. Retired after verified completion. The header-free Create alternative is only an exact completed retry authenticated using the request's administrator/password, not unauthenticated setup.",
                ))),
            );
    }
}

#[cfg(test)]
#[path = "api_create_openapi_tests.rs"]
mod openapi_tests;

#[cfg(all(test, unix))]
pub(crate) mod tests {
    use std::{fs, os::unix::fs::PermissionsExt};

    use std::sync::{Arc, Mutex};

    use api::SetupAdmission;
    use axum::{
        Json,
        extract::State,
        http::{HeaderMap, StatusCode},
    };
    use tokio::sync::{RwLock, mpsc};
    use tokio_util::sync::CancellationToken;

    use super::{
        Active, CreateIntent, CreateRequest, CreateResult, SetupIntent, Shared, cancel, create,
    };

    #[tokio::test]
    async fn owner_routes_stay_mounted_when_runtime_becomes_available_or_unavailable() {
        use axum::{
            Router,
            body::Body,
            extract::Path,
            http::Request,
            routing::{delete, get},
        };
        use idp_model::model::Id;
        use tower::ServiceExt;

        let root = std::env::temp_dir().join(format!(
            "stable-owner-routes-{}",
            idp_model::model::Id::now_v7()
        ));
        let host = crate::BootstrapConfig::create_http(
            &root,
            crate::BootstrapPurpose::AwaitingChoice,
            None,
            "127.0.0.1:3000".parse().expect("HTTP backend"),
            "https://unified.localhost:1355".into(),
        )
        .expect("neutral host");
        let worker = crate::SetupHost::create_http(host)
            .expect("HTTP setup")
            .into_worker(CancellationToken::new());
        let router = worker.router().expect("stable router");
        let owner = Router::new()
            .route(
                "/idp/.well-known/openid-configuration",
                get(|| async { "discovery" }),
            )
            .route(
                "/idp/users/{user_id}",
                get(|Path(_id): Path<Id>| async { StatusCode::UNAUTHORIZED }),
            )
            .route(
                "/idp/users/{user_id}/consents/{consent_id}",
                delete(|Path((_user, _consent)): Path<(Id, Id)>| async { StatusCode::NO_CONTENT }),
            )
            .route("/storage", get(|| async { StatusCode::OK }));
        let user = Id::now_v7();
        let consent = Id::now_v7();
        for available in [false, true, false] {
            *worker.shared.normal.write().await = available.then(|| owner.clone());
            for (method, path, ready_status) in [
                (
                    "GET",
                    "/idp/.well-known/openid-configuration".to_owned(),
                    StatusCode::OK,
                ),
                (
                    "GET",
                    format!("/idp/users/{user}"),
                    StatusCode::UNAUTHORIZED,
                ),
                (
                    "DELETE",
                    format!("/idp/users/{user}/consents/{consent}"),
                    StatusCode::NO_CONTENT,
                ),
                ("GET", "/storage".to_owned(), StatusCode::OK),
            ] {
                let response = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method(method)
                            .uri(path)
                            .body(Body::empty())
                            .expect("owner request"),
                    )
                    .await
                    .expect("owner response");
                assert_eq!(
                    response.status(),
                    if available {
                        ready_status
                    } else {
                        StatusCode::SERVICE_UNAVAILABLE
                    }
                );
            }
            for path in [
                "/idp/version",
                "/management/openapi.json",
                "/storage/openapi.json",
            ] {
                let response = router
                    .clone()
                    .oneshot(
                        Request::get(path)
                            .body(Body::empty())
                            .expect("metadata request"),
                    )
                    .await
                    .expect("metadata response");
                assert_eq!(response.status(), StatusCode::OK);
            }
            for path in ["/idp/nonexistent", "/storage/"] {
                let response = router
                    .clone()
                    .oneshot(
                        Request::get(path)
                            .body(Body::empty())
                            .expect("unknown request"),
                    )
                    .await
                    .expect("unknown response");
                assert_eq!(response.status(), StatusCode::NOT_FOUND);
            }
        }
        drop(worker);
        std::fs::remove_dir_all(root).expect("isolated cleanup");
    }

    pub(crate) async fn status_tracks_revocation_in_existing_runtime(
        runtime: crate::UnifiedRuntime,
        local: crate::LocalProvisioner,
        tls: &crate::LocalTls,
        root: &std::path::Path,
        role: idp_model::model::Id,
    ) {
        use axum::{
            body::{Body, to_bytes},
            http::Request,
        };
        use tower::ServiceExt;

        async fn observe(
            router: &axum::Router,
            expected: StatusCode,
            ready: bool,
        ) -> serde_json::Value {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/idp/setup/status")
                        .body(Body::empty())
                        .expect("status request"),
                )
                .await
                .expect("production status router");
            assert_eq!(response.status(), expected);
            let body: serde_json::Value = serde_json::from_slice(
                &to_bytes(response.into_body(), 2048)
                    .await
                    .expect("status body"),
            )
            .expect("status JSON");
            assert_eq!(body["ready"], ready);
            if !ready {
                assert!(
                    body.get("login_client").is_none(),
                    "unready status must not advertise login readiness"
                );
            }
            body
        }

        let policy = runtime.full_node_test_management();
        let service = management_service::ManagementService::new(
            management_service::replica::DbPermissionRepo::new(policy.clone()),
            management_service::replica::DbRoleRepo::new(policy),
        );
        let application = idp_model::contract::INSTALLATION_POLICY_ID;
        let permission = service
            .list_permissions(application, 0, 128)
            .await
            .expect("real public permission inventory")
            .into_iter()
            .find(|permission| {
                permission.name == idp_model::contract::IdentityAction::IdpSecuritySync.permission()
            })
            .expect("required IdP context grant")
            .id;
        let mut host =
            crate::BootstrapConfig::create(root, crate::BootstrapPurpose::AwaitingChoice, None)
                .expect("isolated status host metadata");
        host.begin_setup().expect("host setup history");
        let admission = Arc::new(
            SetupAdmission::open(&root.join("setup-admission")).expect("protected admission"),
        );
        let retained =
            fs::read_to_string(root.join("setup-admission/state")).expect("protected retrieval");
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Setup {}", retained.lines().nth(1).expect("code"))
                .parse()
                .expect("setup header"),
        );
        let intent = CreateIntent {
            operation_nonce: "0123456789abcdef0123456789abcdef".into(),
            administrator: "operator".into(),
            node: "HTTPS source".into(),
        }
        .canonical()
        .expect("completed result intent");
        admission
            .bind(&headers, &intent)
            .expect("retained exact setup result");
        admission
            .retire(&headers, &intent)
            .expect("retired setup admission");
        let mut coordinator =
            super::CreateHost::new(host, tls.clone(), admission, CancellationToken::new());
        let router = coordinator.router().expect("production setup router");
        coordinator
            .publish(runtime)
            .await
            .expect("publish only currently ready real owners");
        local.mark_complete().expect("real source owner completion");
        coordinator.local = Some(local);
        *coordinator.shared.view.write().await = coordinator
            .ready_result()
            .expect("verified public login result");
        let completed = observe(&router, StatusCode::OK, true).await;

        service
            .remove_permission_from_role(application, role, permission)
            .await
            .expect("learn local context-grant revocation through owner facade");
        observe(&router, StatusCode::SERVICE_UNAVAILABLE, false).await;
        let normal = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/management/replication/devices/untrusted/selections")
                    .body(Body::empty())
                    .expect("normal protected request"),
            )
            .await
            .expect("normal owner router");
        assert_eq!(
            normal.status(),
            StatusCode::UNAUTHORIZED,
            "readiness failure is not a blanket route gate"
        );
        service
            .add_permission_to_role(application, role, permission)
            .await
            .expect("authorized local grant restoration");
        let restored = observe(&router, StatusCode::OK, true).await;
        assert_eq!(
            restored["login_client"], completed["login_client"],
            "status recomputes readiness without replacing the completed identity"
        );
        coordinator
            .unpublish()
            .await
            .expect("coordinated real owner cleanup");
        observe(&router, StatusCode::SERVICE_UNAVAILABLE, false).await;
        drop(router);
        drop(coordinator);
        fs::remove_dir_all(root).expect("isolated status metadata cleanup");
    }

    #[tokio::test]
    async fn cancellation_and_retirement_have_one_winner_and_finalizing_rejects_submissions() {
        let root = std::env::temp_dir().join(format!(
            "setup-finalization-{}",
            idp_model::model::Id::now_v7()
        ));
        crate::local_provisioning::create_private_directory(&root, false)
            .expect("isolated private parent");
        let admission =
            Arc::new(SetupAdmission::open(&root.join("admission")).expect("protected admission"));
        let retained =
            fs::read_to_string(root.join("admission/state")).expect("protected retrieval");
        let code = retained.lines().nth(1).expect("code");
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Setup {code}").parse().expect("header"),
        );
        let intent = CreateIntent {
            operation_nonce: "0123456789abcdef0123456789abcdef".into(),
            administrator: "operator".into(),
            node: "Test Node".into(),
        };
        let canonical = intent.canonical().expect("exact intent");
        admission
            .bind(&headers, &canonical)
            .expect("durable acceptance");
        let (sender, mut commands) = mpsc::channel(1);
        let shared = Arc::new(Shared {
            admission,
            commands: sender,
            view: Arc::new(RwLock::new(CreateResult::unfinished("creating"))),
            normal: RwLock::new(None),
            unavailable: axum::Router::new(),
            readiness: RwLock::new(None),
            active: Mutex::new(None),
            shutdown: CancellationToken::new(),
        });

        let cancelled = shared.shutdown.child_token();
        *shared.active.lock().expect("active lock") = Some(Active {
            intent: canonical.clone(),
            cancellation: cancelled.clone(),
            cancellable: true,
        });
        assert_eq!(
            cancel(
                State(shared.clone()),
                headers.clone(),
                Json(SetupIntent::Create(intent.clone()))
            )
            .await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            shared.begin_retirement(&canonical),
            Err(StatusCode::CONFLICT),
            "a 204 cancellation wins before retirement can commit"
        );
        assert!(cancelled.is_cancelled());
        assert!(!shared.admission.is_retired().expect("retained admission"));

        let finalizing = shared.shutdown.child_token();
        *shared.active.lock().expect("active lock") = Some(Active {
            intent: canonical.clone(),
            cancellation: finalizing.clone(),
            cancellable: true,
        });
        shared
            .begin_retirement(&canonical)
            .expect("retirement wins the active lock");
        assert!(
            !shared
                .admission
                .is_retired()
                .expect("retirement not yet persisted")
        );
        assert_eq!(
            cancel(
                State(shared.clone()),
                headers.clone(),
                Json(SetupIntent::Create(intent.clone()))
            )
            .await,
            StatusCode::CONFLICT,
            "cancel cannot return 204 in the completion-to-retirement window"
        );
        assert!(!finalizing.is_cancelled());
        for node in ["Test Node", "Competing Node"] {
            let request = CreateRequest {
                operation_nonce: intent.operation_nonce.clone(),
                administrator: intent.administrator.clone(),
                node: node.into(),
                password: "protected-test-password".into(),
            };
            assert_eq!(
                create(State(shared.clone()), headers.clone(), Json(request))
                    .await
                    .status(),
                StatusCode::CONFLICT
            );
            assert!(
                matches!(commands.try_recv(), Err(mpsc::error::TryRecvError::Empty)),
                "finalizing submissions must not queue behind retirement"
            );
        }
        shared
            .admission
            .retire(&headers, &canonical)
            .expect("persist winning retirement");
        assert!(shared.admission.is_retired().expect("retired"));
        shared.shutdown.cancel();
        assert!(
            finalizing.is_cancelled(),
            "host shutdown still signals owner cleanup after retirement starts"
        );
        drop(shared);
        fs::remove_dir_all(root).expect("isolated cleanup");
    }

    #[tokio::test]
    async fn request_drop_does_not_cancel_host_work_but_authorized_cancel_does() {
        let root =
            std::env::temp_dir().join(format!("create-cancel-{}", idp_model::model::Id::now_v7()));
        fs::create_dir(&root).expect("isolated admission parent");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private parent");
        let admission =
            Arc::new(SetupAdmission::open(&root.join("admission")).expect("host admission"));
        let retained =
            fs::read_to_string(root.join("admission/state")).expect("protected retrieval");
        let code = retained.lines().nth(1).expect("generated code");
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Setup {code}")
                .parse()
                .expect("authorization header"),
        );
        let (sender, mut commands) = mpsc::channel(1);
        let shared = Arc::new(Shared {
            admission,
            commands: sender,
            view: Arc::new(RwLock::new(CreateResult::unfinished("awaiting_choice"))),
            normal: RwLock::new(None),
            unavailable: axum::Router::new(),
            readiness: RwLock::new(None),
            active: Mutex::new(None),
            shutdown: CancellationToken::new(),
        });
        let intent = CreateIntent {
            operation_nonce: "0123456789abcdef0123456789abcdef".into(),
            administrator: "operator".into(),
            node: "Test Node".into(),
        };
        let request = CreateRequest {
            operation_nonce: intent.operation_nonce.clone(),
            administrator: intent.administrator.clone(),
            node: intent.node.clone(),
            password: "protected-test-password".into(),
        };
        let caller = tokio::spawn(create(
            State(shared.clone()),
            headers.clone(),
            Json(request),
        ));
        let command = tokio::time::timeout(std::time::Duration::from_secs(1), commands.recv())
            .await
            .expect("bounded command admission")
            .expect("queued host command");
        caller.abort();
        assert!(caller.await.expect_err("request canceled").is_cancelled());
        assert!(
            !command.cancellation.is_cancelled(),
            "request lifetime is not setup lifetime"
        );
        assert_eq!(
            cancel(
                State(shared.clone()),
                headers.clone(),
                Json(SetupIntent::Create(intent.clone()))
            )
            .await,
            StatusCode::CONFLICT,
            "no active accepted work cannot report successful cancellation"
        );
        shared
            .admission
            .bind(&headers, &command.intent)
            .expect("durable owner acceptance");
        *shared.active.lock().expect("active lock") = Some(Active {
            intent: command.intent.clone(),
            cancellation: command.cancellation.clone(),
            cancellable: true,
        });
        assert_eq!(
            cancel(
                State(shared.clone()),
                HeaderMap::new(),
                Json(SetupIntent::Create(intent.clone()))
            )
            .await,
            StatusCode::UNAUTHORIZED
        );
        assert!(!command.cancellation.is_cancelled());
        assert_eq!(
            cancel(
                State(shared.clone()),
                headers,
                Json(SetupIntent::Create(intent))
            )
            .await,
            StatusCode::NO_CONTENT
        );
        assert!(command.cancellation.is_cancelled());
        drop(command);
        drop(shared);
        fs::remove_dir_all(root).expect("isolated cleanup");
    }
}

pub fn setup_openapi() -> utoipa::openapi::OpenApi {
    CreateDoc::openapi()
}

#[utoipa::path(get, path = "/idp/setup/openapi.json", operation_id = "setup_openapi", responses((status = 200, description = "Complete Setup contract")))]
async fn setup_openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(CreateDoc::openapi())
}
