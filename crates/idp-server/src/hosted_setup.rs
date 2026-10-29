use std::{
    fs,
    future::Future,
    io::{self, Write},
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
};

use api::{
    SetupAdmission, SetupAdmissionError,
    setup_files::{present, private_options, read_private, require_absent, sync_ancestry},
};
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{MethodFilter, get, on, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use tower_http::cors::CorsLayer;
use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
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
    pub fn canonical(&self) -> Result<String, StatusCode> {
        validate_human_intent(&self.operation_nonce, &[&self.administrator, &self.node])?;
        serde_json::to_string(&(
            "create",
            ["idp", "device", "storage"],
            &self.operation_nonce,
            &self.administrator,
            &self.node,
        ))
        .map_err(|_| StatusCode::BAD_REQUEST)
    }
}

impl CreateRequest {
    pub fn intent(&self) -> CreateIntent {
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
    pub fn unfinished(state: &str) -> Self {
        Self {
            stage: "installation".into(),
            state: state.into(),
            ready: false,
            owners: vec!["idp".into(), "device".into(), "storage".into()],
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
    pub fn validate(&self) -> Result<(), StatusCode> {
        validate_human_intent(&self.operation_nonce, &[&self.node])?;
        let peer = self
            .peer
            .parse::<iroh::EndpointId>()
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        if peer.to_string() != self.peer {
            return Err(StatusCode::BAD_REQUEST);
        }
        device_service::hosted_join::NodeJoinRequest::validate_trusted_peer(&self.peer)
            .map_err(|_| StatusCode::BAD_REQUEST)
    }
}

impl JoinIntent {
    pub fn request(&self) -> JoinRequest {
        JoinRequest {
            operation_nonce: self.operation_nonce.clone(),
            node: self.node.clone(),
            peer: self.peer.clone(),
        }
    }

    pub fn matches(&self, request: &JoinRequest) -> bool {
        self.operation_nonce == request.operation_nonce
            && self.node == request.node
            && self.peer == request.peer
    }

    pub fn canonical(&self) -> Result<String, StatusCode> {
        self.request().validate()?;
        device_service::issuer_discovery::validate_issuer(&self.issuer)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        serde_json::to_string(&(
            "join",
            ["idp", "device", "storage"],
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
    pub fn canonical(&self) -> Result<String, StatusCode> {
        match self {
            Self::Create(intent) => intent.canonical(),
            Self::Join(intent) => intent.canonical(),
        }
    }

    pub fn decode(retained: &str) -> Result<Self, StatusCode> {
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

pub enum SetupOperation {
    Create(CreateRequest),
    Join(JoinRequest),
}

pub struct SetupCommand {
    pub request: SetupOperation,
    pub intent: String,
    headers: HeaderMap,
    pub cancellation: CancellationToken,
    reply: Option<oneshot::Sender<Result<CreateResult, StatusCode>>>,
}

struct Active {
    intent: String,
    cancellation: CancellationToken,
    cancellable: bool,
}

struct Shared {
    admission: Arc<SetupAdmission>,
    commands: mpsc::Sender<SetupCommand>,
    view: Arc<RwLock<CreateResult>>,
    normal: RwLock<Option<Router>>,
    unavailable: Router,
    readiness: RwLock<Option<SetupReadiness>>,
    active: Mutex<Option<Active>>,
    shutdown: CancellationToken,
    // Last field: channel closure must follow admission and router-state destruction.
    released: watch::Sender<()>,
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

pub type SetupReadiness =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = io::Result<bool>> + Send>> + Send + Sync>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SetupPurpose {
    AwaitingChoice,
    Create,
    Join,
}

pub struct HostedSetupSettings {
    pub directory: PathBuf,
    pub ui_origin: String,
    pub https: bool,
    pub authentication_base: String,
    pub contracts: Vec<(String, utoipa::openapi::OpenApi)>,
}

pub struct HostedSetupReady {
    pub router: Router,
    pub readiness: SetupReadiness,
    pub login_client: LoginClient,
}

/// Opens and coordinates concrete owner runtimes without owning setup admission.
/// Prepared owners must be served through `control` before completion checks that use HTTP.
/// The worker alone admits operations and retires their capability after verified completion.
pub trait HostedSetupOwners: Send + 'static {
    fn settings(&self) -> HostedSetupSettings;
    fn purpose(&self) -> SetupPurpose;
    fn trusted_peer(&self) -> Option<&str>;
    fn issuer(&self) -> String;
    fn service_router(&self, control: HostedSetupControl) -> Router;
    fn verification_client(&self) -> io::Result<reqwest::Client>;
    fn discover<'a>(
        &'a self,
        peer: &'a str,
        cancellation: &'a CancellationToken,
    ) -> impl Future<Output = io::Result<String>> + Send + 'a;
    fn reopen(
        &mut self,
        retired: bool,
        intent: Option<JoinIntent>,
        control: HostedSetupControl,
    ) -> impl Future<Output = io::Result<Option<HostedSetupReady>>> + Send;
    fn provision(
        &mut self,
        command: &SetupCommand,
        control: HostedSetupControl,
    ) -> impl Future<Output = Result<HostedSetupReady, StatusCode>> + Send;
    fn admit(&mut self, _command: &SetupCommand) -> Result<(), StatusCode> {
        Ok(())
    }
    fn shutdown(&mut self) -> impl Future<Output = io::Result<()>> + Send;
}

#[derive(Clone)]
pub struct HostedSetupControl(Arc<Shared>);

pub struct HostedSetup<B: HostedSetupOwners> {
    owners: B,
    settings: HostedSetupSettings,
    shared: Arc<Shared>,
    commands: mpsc::Receiver<SetupCommand>,
}

impl HostedSetupControl {
    pub fn new(
        admission: Arc<SetupAdmission>,
        state: &str,
        contracts: Vec<(String, utoipa::openapi::OpenApi)>,
        shutdown: CancellationToken,
    ) -> (Self, mpsc::Receiver<SetupCommand>) {
        let (commands, receiver) = mpsc::channel(1);
        let view = Arc::new(RwLock::new(CreateResult::unfinished(state)));
        let unavailable = owner_routes(contracts).with_state(view.clone());
        (
            Self(Arc::new(Shared {
                admission,
                commands,
                view,
                normal: RwLock::new(None),
                unavailable,
                readiness: RwLock::new(None),
                active: Mutex::new(None),
                shutdown,
                released: watch::channel(()).0,
            })),
            receiver,
        )
    }
    pub fn router(&self, services: Router) -> Router {
        services.fallback_service(Router::new().fallback(normal).with_state(self.0.clone()))
    }
    pub async fn serve(&self, router: Router, readiness: SetupReadiness) {
        *self.0.normal.write().await = Some(router);
        *self.0.readiness.write().await = Some(readiness);
    }
    pub async fn publish(&self, ready: HostedSetupReady) {
        self.serve(ready.router, ready.readiness).await;
        let mut result = CreateResult::unfinished("ready");
        result.ready = true;
        result.login_client = Some(ready.login_client);
        *self.0.view.write().await = result;
    }
    pub async fn withdraw(&self) {
        *self.0.normal.write().await = None;
        *self.0.readiness.write().await = None;
    }
}

/// Builds the IdP-owned setup API before domain owners are initialized.
pub fn build_setup_router(control: HostedSetupControl, api_base: &str, prefix: &str) -> Router {
    let routes = Router::new()
        .route("/setup/status", get(status))
        .route("/setup/create", post(create))
        .route("/setup/join", post(join))
        .route("/setup/cancel", post(cancel))
        .route("/setup/admission", post(admission))
        .route("/setup/intent", post(intent))
        .with_state(control.0);
    let routes = if prefix.is_empty() || prefix == "/" {
        routes
    } else {
        Router::new().nest(prefix, routes)
    };
    routes.merge(crate::public_metadata_router(api_base, prefix))
}

pub fn setup_origin(value: &str, https: bool) -> io::Result<HeaderValue> {
    if !https {
        let url = reqwest::Url::parse(value).map_err(io::Error::other)?;
        if url.scheme() == "http"
            && url.host_str().is_some_and(|host| {
                host == "localhost"
                    || host
                        .trim_matches(['[', ']'])
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
            })
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none()
        {
            return HeaderValue::from_str(value.trim_end_matches('/')).map_err(io::Error::other);
        }
    }
    api::setup_origin(value)
}

impl<B: HostedSetupOwners> HostedSetup<B> {
    pub fn new(owners: B, admission: Arc<SetupAdmission>, shutdown: CancellationToken) -> Self {
        let mut settings = owners.settings();
        let state = match owners.purpose() {
            SetupPurpose::AwaitingChoice => "awaiting_choice",
            SetupPurpose::Create => "creating",
            SetupPurpose::Join => "joining",
        };
        let (control, commands) = HostedSetupControl::new(
            admission,
            state,
            std::mem::take(&mut settings.contracts),
            shutdown,
        );
        Self {
            owners,
            settings,
            shared: control.0,
            commands,
        }
    }
    pub fn control(&self) -> HostedSetupControl {
        HostedSetupControl(self.shared.clone())
    }
    pub fn router(&self) -> io::Result<Router> {
        self.router_for_origin(setup_origin(&self.settings.ui_origin, self.settings.https)?)
    }
    pub fn router_for_origin(&self, origin: HeaderValue) -> io::Result<Router> {
        setup_origin(
            origin.to_str().map_err(io::Error::other)?,
            self.settings.https,
        )?;
        Ok(self
            .owners
            .service_router(HostedSetupControl(self.shared.clone()))
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
            ))
            .fallback_service(
                Router::new()
                    .fallback(normal)
                    .with_state(self.shared.clone())
                    .layer(CorsLayer::very_permissive().allow_private_network(true)),
            ))
    }
    pub async fn initialize(&mut self) {
        let reopen = Box::pin(self.reopen()).await;
        if let Err(error) = reopen {
            if let Err(cleanup) = self.unpublish().await {
                log::info!("Node reopen cleanup failed kind={:?}", cleanup.kind());
            }
            log::info!("Node reopen failed kind={:?}", error.kind());
            *self.shared.view.write().await = CreateResult::unfinished("unavailable");
        } else if self.owners.purpose() != SetupPurpose::AwaitingChoice {
            let mut view = self.shared.view.write().await;
            if !view.ready {
                *view = CreateResult::unfinished("unfinished");
            }
        }
    }

    /// Joins owner cleanup and router teardown; drop issued routers/controls before joining.
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
        self.commands.close();
        while let Some(mut command) = self.commands.recv().await {
            if let Some(reply) = command.reply.take() {
                let _ = reply.send(Err(StatusCode::SERVICE_UNAVAILABLE));
            }
        }
        let result = self.unpublish().await;
        let mut released = self.shared.released.subscribe();
        drop(self.shared);
        let _ = released.changed().await;
        result
    }

    async fn reopen(&mut self) -> io::Result<()> {
        if self.owners.purpose() == SetupPurpose::AwaitingChoice {
            return Ok(());
        }
        let intent = if self.owners.purpose() == SetupPurpose::Join {
            Some(self.retained_join_intent()?)
        } else {
            None
        };
        let admission = self.shared.admission.clone();
        let canonical = intent
            .as_ref()
            .map(JoinIntent::canonical)
            .transpose()
            .map_err(|_| io::Error::other("invalid retained Join admission"))?;
        let retired = tokio::task::spawn_blocking(move || {
            let retired = admission.is_retired().map_err(io::Error::other)?;
            if let Some(intent) = canonical.as_ref()
                && retired
                && !admission
                    .retired_matches(intent)
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
        if let Some(ready) = self.owners.reopen(retired, intent, self.control()).await? {
            HostedSetupControl(self.shared.clone()).publish(ready).await;
        }
        Ok(())
    }
    async fn unpublish(&mut self) -> io::Result<()> {
        HostedSetupControl(self.shared.clone()).withdraw().await;
        self.owners.shutdown().await
    }
    async fn retire(
        &self,
        command: &SetupCommand,
        login: LoginClient,
    ) -> Result<CreateResult, StatusCode> {
        self.shared.begin_retirement(&command.intent)?;
        let admission = self.shared.admission.clone();
        let headers = command.headers.clone();
        let intent = command.intent.clone();
        tokio::task::spawn_blocking(move || admission.retire(&headers, &intent))
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .map_err(admission_status)?;
        let mut result = CreateResult::unfinished("ready");
        result.ready = true;
        result.login_client = Some(login);
        *self.shared.view.write().await = result.clone();
        Ok(result)
    }
    async fn execute(&mut self, command: &mut SetupCommand) -> Result<CreateResult, StatusCode> {
        if command.cancellation.is_cancelled() {
            return Err(StatusCode::CONFLICT);
        }
        if self.shared.view.read().await.ready {
            return self.completed_retry(command).await;
        }
        if let SetupOperation::Join(request) = &command.request {
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
                let issuer = self
                    .owners
                    .discover(&request.peer, &command.cancellation)
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
        self.shared
            .admission
            .check(&command.headers)
            .map_err(admission_status)?;
        self.owners.admit(command)?;
        let admission = self.shared.admission.clone();
        let headers = command.headers.clone();
        let intent = command.intent.clone();
        tokio::task::spawn_blocking(move || admission.bind(&headers, &intent))
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .map_err(admission_status)?;
        if matches!(&command.request, SetupOperation::Join(_)) {
            self.retain_join_intent(&command.intent)
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        }
        *self.shared.active.lock().expect("Create cancellation lock") = Some(Active {
            intent: command.intent.clone(),
            cancellation: command.cancellation.clone(),
            cancellable: true,
        });
        let accepted = CreateResult::unfinished(match &command.request {
            SetupOperation::Create(_) => "creating",
            SetupOperation::Join(_) => "joining",
        });
        *self.shared.view.write().await = accepted.clone();
        if let Some(reply) = command.reply.take() {
            let _ = reply.send(Ok(accepted));
        }
        let started = std::time::Instant::now();
        let result = match self.owners.provision(command, self.control()).await {
            Ok(ready) => {
                *self.shared.normal.write().await = Some(ready.router);
                *self.shared.readiness.write().await = Some(ready.readiness);
                self.retire(command, ready.login_client).await
            }
            Err(error) => Err(error),
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

    fn retain_join_intent(&self, canonical: &str) -> io::Result<()> {
        let path = self
            .settings
            .directory
            .as_path()
            .join("join-admission.json");
        let pending = self
            .settings
            .directory
            .as_path()
            .join("join-admission.pending");
        require_absent(&pending)?;
        if present(&path)? {
            if read_private(&path)? != canonical.as_bytes() {
                return Err(io::Error::other(
                    "Join admission differs from retained host intent",
                ));
            }
            return self.initialize_join_history();
        }
        if self.owners.purpose() != SetupPurpose::AwaitingChoice {
            return Err(io::Error::other(
                "selected Join is missing its retained admission intent",
            ));
        }
        let mut file = private_options(true).open(&pending)?;
        file.write_all(canonical.as_bytes())?;
        file.sync_all()?;
        fs::rename(pending, &path)?;
        sync_ancestry(self.settings.directory.as_path())?;
        if read_private(&path)? != canonical.as_bytes() {
            return Err(io::Error::other("Join admission durable readback mismatch"));
        }
        self.initialize_join_history()
    }

    fn initialize_join_history(&self) -> io::Result<()> {
        for stage in ["credentials", "owner"] {
            let path = self
                .settings
                .directory
                .join(format!("join-{stage}.attempted"));
            if !present(&path)? {
                if self.owners.purpose() != SetupPurpose::AwaitingChoice {
                    return Err(io::Error::other(
                        "selected Join is missing production history",
                    ));
                }
                let mut file = private_options(true).open(&path)?;
                file.write_all(b"unattempted\n")?;
                file.sync_all()?;
                sync_ancestry(self.settings.directory.as_path())?;
            }
            join_attempted(self.settings.directory.as_path(), stage)?;
        }
        Ok(())
    }

    fn retained_join_intent(&self) -> io::Result<JoinIntent> {
        require_absent(
            &self
                .settings
                .directory
                .as_path()
                .join("join-admission.pending"),
        )?;
        let bytes = read_private(
            &self
                .settings
                .directory
                .as_path()
                .join("join-admission.json"),
        )?;
        let canonical = std::str::from_utf8(&bytes).map_err(io::Error::other)?;
        match SetupIntent::decode(canonical)
            .map_err(|_| io::Error::other("invalid retained Join admission"))?
        {
            SetupIntent::Join(intent)
                if Some(intent.peer.as_str()) == self.owners.trusted_peer()
                    && intent.issuer == self.owners.issuer() =>
            {
                Ok(intent)
            }
            _ => Err(io::Error::other(
                "retained Join admission conflicts with host identity",
            )),
        }
    }

    async fn completed_retry(
        &mut self,
        command: &SetupCommand,
    ) -> Result<CreateResult, StatusCode> {
        let SetupOperation::Create(request) = &command.request else {
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
        let result = self.shared.view.read().await.clone();
        let client_id = &result
            .login_client
            .as_ref()
            .expect("ready public login Client")
            .client_id;
        let http = self
            .owners
            .verification_client()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let configuration = of_idp_client::apis::configuration::Configuration {
            base_path: self.settings.authentication_base.clone(),
            client: http,
            ..Default::default()
        };
        let response = of_idp_client::apis::default_api::token(
            &configuration,
            of_idp_client::apis::default_api::TokenParams {
                grant_type: Some("password".to_owned()),
                client_id: Some(client_id.clone()),
                username: Some(request.administrator.clone()),
                password: Some(request.password.clone()),
                scope: Some("openid".to_owned()),
                ..Default::default()
            },
        )
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
        let token: model::contract::TokenResponse = serde_json::from_value(
            serde_json::to_value(response).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        )
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

        if token.access_token.0.is_empty() {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let readiness = self
            .shared
            .readiness
            .read()
            .await
            .clone()
            .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
        if !readiness()
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
            Some(readiness) => matches!(readiness().await, Ok(true)),
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
    submit(shared, headers, SetupOperation::Create(request), intent).await
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
    submit(
        shared,
        headers,
        SetupOperation::Join(request),
        String::new(),
    )
    .await
}

async fn submit(
    shared: Arc<Shared>,
    headers: HeaderMap,
    request: SetupOperation,
    intent: String,
) -> Response {
    let joining = matches!(&request, SetupOperation::Join(_));
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
                SetupOperation::Create(_) => active.intent == intent,
                SetupOperation::Join(request) => {
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
        let command = SetupCommand {
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

fn owner_routes(
    contracts: Vec<(String, utoipa::openapi::OpenApi)>,
) -> Router<Arc<RwLock<CreateResult>>> {
    let mut router = Router::new();
    for (prefix, document) in contracts {
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
}

#[derive(OpenApi)]
#[openapi(
    paths(create, join, status, cancel, admission, intent),
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
#[path = "hosted_setup_openapi_tests.rs"]
mod openapi_tests;

pub fn join_attempted(directory: &std::path::Path, stage: &str) -> io::Result<bool> {
    require_absent(&directory.join(format!("join-{stage}.pending")))?;
    match read_private(&directory.join(format!("join-{stage}.attempted")))?.as_slice() {
        b"unattempted\n" => Ok(false),
        b"attempted\n" => Ok(true),
        _ => Err(io::Error::other("invalid retained Join production history")),
    }
}

pub fn begin_join_attempt(
    directory: &std::path::Path,
    stage: &str,
    root: &std::path::Path,
) -> io::Result<bool> {
    if join_attempted(directory, stage)? {
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
    if !join_attempted(directory, stage)? {
        return Err(io::Error::other(
            "Join production history readback mismatch",
        ));
    }
    Ok(true)
}

pub fn setup_openapi() -> utoipa::openapi::OpenApi {
    CreateDoc::openapi()
}

#[cfg(test)]
mod tests {
    use super::{
        Active, CreateIntent, CreateRequest, CreateResult, SetupIntent, Shared, cancel, create,
    };
    use api::SetupAdmission;
    use axum::{
        Json,
        extract::State,
        http::{HeaderMap, StatusCode},
    };
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        sync::{Arc, Mutex},
    };
    use tokio::sync::{RwLock, mpsc, watch};
    use tokio_util::sync::CancellationToken;
    #[tokio::test]
    async fn cancellation_and_retirement_have_one_winner_and_finalizing_rejects_submissions() {
        let root = std::env::temp_dir().join(format!(
            "setup-finalization-{}",
            idp_model::model::Id::now_v7()
        ));
        api::setup_files::create_private_directory(&root, false).expect("isolated private parent");
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
            released: watch::channel(()).0,
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
            released: watch::channel(()).0,
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
