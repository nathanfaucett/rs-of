use std::{
    future::Future,
    io::{self, Write},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, RwLock},
};

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{
        HeaderMap, Method, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use tower_http::cors::CorsLayer;

use crate::{
    IndependentCreateIntent, IndependentCreateRequest, IndependentOwnerPublic,
    IndependentSetupHost, LocalTls, SetupAdmission,
    setup_files::{present, private_options, read_private, sync_ancestry},
    setup_origin, setup_origin_guard,
};

pub type IndependentRegistrar = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send>> + Send + Sync,
>;
pub type IndependentSecretLoader = Arc<dyn Fn(&str) -> io::Result<String> + Send + Sync>;
type IndependentReadiness = Arc<
    dyn Fn(IndependentCreateControl) -> Pin<Box<dyn Future<Output = io::Result<()>> + Send>>
        + Send
        + Sync,
>;

pub struct IndependentCreateCommand {
    pub intent: IndependentCreateIntent,
    pub password: Option<String>,
    pub cancellation: CancellationToken,
    headers: HeaderMap,
}

pub(crate) struct CreateLifecycle {
    pub(crate) intent: Option<IndependentCreateIntent>,
    pub(crate) active: Option<CancellationToken>,
    pub(crate) view: &'static str,
    pub(crate) public: Option<IndependentOwnerPublic>,
    pub(crate) registrar: Option<IndependentRegistrar>,
    pub(crate) loader: Option<IndependentSecretLoader>,
    pub(crate) challenges: Vec<crate::independent_exchange::OutstandingChallenge>,
    pub(crate) history: Option<(String, serde_json::Value, mpsc::Sender<serde_json::Value>)>,
}

pub(crate) struct CreateShared {
    pub(crate) owner: String,
    pub(crate) base: String,
    pub(crate) directory: PathBuf,
    pub(crate) admission: Arc<SetupAdmission>,
    pub(crate) lifecycle: Mutex<CreateLifecycle>,
    normal: RwLock<Option<Router>>,
    readiness: RwLock<Option<IndependentReadiness>>,
    commands: mpsc::Sender<IndependentCreateCommand>,
    pub(crate) shutdown: CancellationToken,
}

#[derive(Clone)]
pub struct IndependentCreateControl(pub(crate) Arc<CreateShared>);

pub struct IndependentCreateHost {
    host: IndependentSetupHost,
    control: IndependentCreateControl,
}

impl IndependentSetupHost {
    pub fn into_create(
        self,
        shutdown: CancellationToken,
    ) -> io::Result<(
        IndependentCreateHost,
        mpsc::Receiver<IndependentCreateCommand>,
    )> {
        let intent = self
            .admission
            .host_intent()
            .map_err(io::Error::other)?
            .map(|value| serde_json::from_str::<IndependentCreateIntent>(&value))
            .transpose()?;
        let base = format!(
            "{}{}",
            LocalTls::api_base(self.local_addr())?,
            self.settings.prefix
        );
        if let Some(intent) = intent.as_ref() {
            intent.canonical()?;
            if intent.base(&self.settings.owner)? != base {
                return Err(io::Error::other("retained owner base disagrees"));
            }
        }
        let directory = self.settings.domain.with_extension("setup-host");
        let record = directory.join("worker.json");
        if present(&record)? && intent.is_none() {
            return Err(io::Error::other(
                "worker evidence has no exact admitted operation",
            ));
        }
        if (present(&self.settings.domain)? || present(&directory.join("initialized.json"))?)
            && !present(&record)?
        {
            return Err(io::Error::other(
                "initialized owner has no exact worker evidence",
            ));
        }
        if self.admission.is_retired().map_err(io::Error::other)?
            && (!present(&record)? || intent.is_none())
        {
            return Err(io::Error::other("retired setup has missing owner evidence"));
        }
        let view = if intent.is_some() {
            "unfinished"
        } else {
            "awaiting_choice"
        };
        let (commands, receiver) = mpsc::channel(1);
        let shared = Arc::new(CreateShared {
            owner: self.settings.owner.clone(),
            base,
            directory,
            admission: self.admission.clone(),
            lifecycle: Mutex::new(CreateLifecycle {
                intent,
                active: None,
                view,
                public: None,
                registrar: None,
                loader: None,
                challenges: Vec::new(),
                history: None,
            }),
            normal: RwLock::new(None),
            readiness: RwLock::new(None),
            commands,
            shutdown,
        });
        Ok((
            IndependentCreateHost {
                host: self,
                control: IndependentCreateControl(shared),
            },
            receiver,
        ))
    }
}

impl IndependentCreateHost {
    pub fn control(&self) -> IndependentCreateControl {
        self.control.clone()
    }
    pub fn router(&self) -> Router {
        let origin = setup_origin(&self.host.settings.origin).expect("validated host origin");
        let normal_cors = CorsLayer::new()
            .allow_origin(origin.clone())
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_headers([AUTHORIZATION, CONTENT_TYPE])
            .allow_private_network(true);
        let setup = Router::new()
            .route("/setup/status", get(status))
            .route("/setup/admission", post(admission))
            .route("/setup/create", post(create))
            .route("/setup/join", post(join))
            .route(
                "/setup/independent/history",
                post(crate::independent_history::challenge),
            )
            .route("/setup/intent", post(recover))
            .route("/setup/cancel", post(cancel))
            .merge(crate::independent_exchange::routes())
            .with_state(self.control.clone())
            .layer(middleware::from_fn_with_state(origin, setup_origin_guard));
        let setup = if self.host.settings.prefix.is_empty() {
            setup
        } else {
            Router::new().nest(&self.host.settings.prefix, setup)
        };
        setup
            .fallback(normal)
            .with_state(self.control.clone())
            .layer(normal_cors)
    }
    pub async fn serve(self) -> io::Result<()> {
        let router = self.router();
        let tls = self.host.tls.clone();
        let address = self.host.local_addr();
        let path = format!("{}/setup/status", self.host.settings.prefix);
        let admission = self.host.admission_path();
        let shutdown = self.control.0.shutdown.clone();
        crate::serve_setup(self, tls, address, router, path, admission, shutdown).await
    }
}

impl IndependentCreateControl {
    pub fn shutdown_token(&self) -> CancellationToken {
        self.0.shutdown.child_token()
    }
    pub fn owner(&self) -> &str {
        &self.0.owner
    }
    pub fn directory(&self) -> &Path {
        &self.0.directory
    }
    pub async fn intent(&self) -> Option<IndependentCreateIntent> {
        self.0.lifecycle.lock().await.intent.clone()
    }
    pub fn read_record<T: DeserializeOwned>(&self) -> io::Result<Option<T>> {
        let path = self.directory().join("worker.json");
        if !present(&path)? {
            if present(&self.directory().join("initialized.json"))? {
                return Err(io::Error::other("initialized owner lost worker evidence"));
            }
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&read_private(&path)?)?))
    }
    pub fn write_record<T: Serialize>(&self, value: &T) -> io::Result<()> {
        let initialized = self.directory().join("initialized.json");
        if present(&initialized)? {
            if !present(&self.directory().join("worker.json"))? {
                return Err(io::Error::other("initialized owner lost worker evidence"));
            }
        } else {
            self.write_document("initialized.json", &true)?;
        }
        self.write_document("worker.json", value)
    }
    pub(crate) fn write_document<T: Serialize>(&self, name: &str, value: &T) -> io::Result<()> {
        let path = self.directory().join(format!(
            ".worker-{}",
            crate::independent_exchange::random_capability()?
        ));
        let mut file = private_options(true).open(&path)?;
        file.write_all(&serde_json::to_vec(value)?)?;
        file.sync_all()?;
        std::fs::rename(path, self.directory().join(name))?;
        sync_ancestry(self.directory())
    }
    pub async fn set_public(
        &self,
        mut public: IndependentOwnerPublic,
        loader: IndependentSecretLoader,
    ) -> io::Result<()> {
        let mut state = self.0.lifecycle.lock().await;
        if state.intent.as_ref() != Some(&public.operation)
            || public.owner != self.owner()
            || public.clients.iter().any(|r| !r["client_secret"].is_null())
        {
            return Err(io::Error::other(
                "public owner evidence disagrees with exact operation",
            ));
        }
        public.registered.clear();
        for registration in &public.clients {
            if self.registration_confirmed(&public.operation, registration)? {
                public.registered.push(
                    registration["client_id"]
                        .as_str()
                        .ok_or_else(|| io::Error::other("missing Client ID"))?
                        .into(),
                );
            }
        }
        state.public = Some(public);
        state.loader = Some(loader);
        Ok(())
    }
    pub async fn set_registrar(&self, registrar: IndependentRegistrar) {
        self.0.lifecycle.lock().await.registrar = Some(registrar);
    }
    pub async fn registered(&self, client_id: &str) -> bool {
        self.0
            .lifecycle
            .lock()
            .await
            .public
            .as_ref()
            .is_some_and(|p| p.registered.iter().any(|id| id == client_id))
    }
    /// The supplied control avoids retaining the host in its own observation callback.
    pub fn set_readiness<F, Check>(&self, check: F) -> io::Result<()>
    where
        F: Fn(IndependentCreateControl) -> Check + Send + Sync + 'static,
        Check: Future<Output = io::Result<()>> + Send + 'static,
    {
        *self
            .0
            .readiness
            .write()
            .map_err(|_| io::Error::other("owner readiness lock unavailable"))? =
            Some(Arc::new(move |control| Box::pin(check(control))));
        Ok(())
    }
    async fn check_readiness(&self) -> io::Result<()> {
        let check = self
            .0
            .readiness
            .read()
            .map_err(|_| io::Error::other("owner readiness lock unavailable"))?
            .clone()
            .ok_or_else(|| io::Error::other("current owner readiness missing"))?;
        if self.0.shutdown.is_cancelled() {
            return Err(io::Error::other("owner is shutting down"));
        }
        check(self.clone()).await
    }
    pub async fn publish_serving(&self, router: Router) -> io::Result<()> {
        *self
            .0
            .normal
            .write()
            .map_err(|_| io::Error::other("owner router lock unavailable"))? = Some(router);
        Ok(())
    }
    pub async fn restore(&self, router: Router) -> io::Result<()> {
        let mut state = self.0.lifecycle.lock().await;
        self.publish_serving(router).await?;
        state.view = if self.0.admission.is_retired().map_err(io::Error::other)? {
            "ready"
        } else {
            "unfinished"
        };
        Ok(())
    }
    pub async fn finish<T: Serialize>(
        &self,
        command: &IndependentCreateCommand,
        router: Router,
        record: &T,
    ) -> io::Result<()> {
        let mut state = self.0.lifecycle.lock().await;
        if command.cancellation.is_cancelled()
            || self.0.shutdown.is_cancelled()
            || state.intent.as_ref() != Some(&command.intent)
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "owner setup cancelled before finalization",
            ));
        }
        self.write_record(record)?;
        self.0
            .admission
            .retire(&command.headers, &command.intent.canonical()?)
            .map_err(io::Error::other)?;
        self.publish_serving(router).await?;
        state.view = "ready";
        state.active = None;
        state.challenges.clear();
        state.history = None;
        state.loader = None;
        state.registrar = None;
        Ok(())
    }
    pub async fn unfinished(&self) {
        let mut state = self.0.lifecycle.lock().await;
        if let Some(active) = state.active.take() {
            active.cancel();
        }
        state.view = "unfinished";
        state.challenges.clear();
        state.history = None;
        state.loader = None;
        state.registrar = None;
    }
    pub async fn wait_peer(
        &self,
        intent: &IndependentCreateIntent,
        owner: &str,
        cancellation: &CancellationToken,
    ) -> io::Result<IndependentOwnerPublic> {
        loop {
            if cancellation.is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "owner setup cancelled",
                ));
            }
            match self.peer(intent, owner).await {
                Ok(peer) => return Ok(peer),
                Err(error) if error.kind() == io::ErrorKind::InvalidInput => return Err(error),
                Err(_) => {
                    tokio::select! { () = cancellation.cancelled() => return Err(io::Error::new(io::ErrorKind::Interrupted, "owner setup cancelled")), () = tokio::time::sleep(std::time::Duration::from_millis(100)) => () }
                }
            }
        }
    }
    pub async fn wait_registered(
        &self,
        client_id: &str,
        cancellation: &CancellationToken,
    ) -> io::Result<()> {
        loop {
            if cancellation.is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "owner setup cancelled",
                ));
            }
            if self.registered(client_id).await {
                return Ok(());
            }
            tokio::select! { () = cancellation.cancelled() => return Err(io::Error::new(io::ErrorKind::Interrupted, "owner setup cancelled")), () = tokio::time::sleep(std::time::Duration::from_millis(50)) => () }
        }
    }
    pub async fn peer(
        &self,
        intent: &IndependentCreateIntent,
        owner: &str,
    ) -> io::Result<IndependentOwnerPublic> {
        let response = LocalTls::verification_client()?
            .get(format!("{}/setup/independent/public", intent.base(owner)?))
            .send()
            .await
            .map_err(|_| io::Error::other("owner HTTPS discovery unavailable"))?;
        if response.status() != StatusCode::OK {
            return Err(io::Error::other(
                "owner has not admitted the exact operation",
            ));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| io::Error::other("owner discovery unavailable"))?;
        if bytes.len() > 65536 {
            return Err(io::Error::other("owner discovery is too large"));
        }
        let public: IndependentOwnerPublic = serde_json::from_slice(&bytes)?;
        if public.operation != *intent
            || public.owner != owner
            || public.clients.iter().any(|r| !r["client_secret"].is_null())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "owner discovery disagrees with exact operation",
            ));
        }
        Ok(public)
    }
}

#[utoipa::path(post, tag = "default", path = "/setup/admission", responses((status = 204, description = "This owner code is currently admitted"), (status = 401, description = "Code invalid or retired"), (status = 503, description = "Admission unavailable")), security(("setup_code" = [])))]
pub(crate) async fn admission(
    State(control): State<IndependentCreateControl>,
    headers: HeaderMap,
) -> Response {
    match control.0.admission.check(&headers) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error.into_response(),
    }
}
#[utoipa::path(post, tag = "default", path = "/setup/create", request_body = IndependentCreateRequest, responses((status = 202, description = "Exact Create accepted; empty body, not completion"), (status = 400, description = "Invalid tuple or password"), (status = 401, description = "Code invalid or retired"), (status = 409, description = "Conflicting retained operation"), (status = 503, description = "Owner unavailable")), security(("setup_code" = [])))]
pub(crate) async fn create(
    State(control): State<IndependentCreateControl>,
    headers: HeaderMap,
    Json(request): Json<IndependentCreateRequest>,
) -> Response {
    if request.intent.purpose != "create" {
        return StatusCode::BAD_REQUEST.into_response();
    }
    accept(control, headers, request).await
}

#[utoipa::path(post, tag = "default", path = "/setup/join", request_body = IndependentCreateRequest, responses((status = 202, description = "Exact Join accepted after authenticated discovery; empty body"), (status = 400, description = "Invalid tuple; Join forbids password and administrator"), (status = 401, description = "Code invalid or retired"), (status = 409, description = "Conflicting retained operation"), (status = 502, description = "Pinned peer discovery failed"), (status = 503, description = "Owner unavailable")), security(("setup_code" = [])))]
pub(crate) async fn join(
    State(control): State<IndependentCreateControl>,
    headers: HeaderMap,
    Json(mut request): Json<IndependentCreateRequest>,
) -> Response {
    if let Err(error) = control.0.admission.check(&headers) {
        return error.into_response();
    }
    if request.intent.purpose != "join" || request.password.is_some() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if let Some(retained) = control.intent().await {
        if request.intent.issuer.is_empty() {
            request.intent.issuer = retained.issuer;
        }
    } else {
        let Some(choice) = request.intent.peer.as_deref() else {
            return StatusCode::BAD_REQUEST.into_response();
        };
        let peer = match crate::IndependentPeer::discover(choice).await {
            Ok(peer) => peer,
            Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
        };
        if !request.intent.issuer.is_empty() && request.intent.issuer != peer.issuer {
            return StatusCode::CONFLICT.into_response();
        }
        request.intent.issuer = peer.issuer;
    }
    accept(control, headers, request).await
}

async fn accept(
    control: IndependentCreateControl,
    headers: HeaderMap,
    request: IndependentCreateRequest,
) -> Response {
    if let Err(error) = control.0.admission.check(&headers) {
        return error.into_response();
    }
    let intent = request.intent;
    let canonical = match intent.canonical() {
        Ok(value) => value,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if intent.base(control.owner()).ok() != Some(control.0.base.as_str())
        || (control.owner() == "idp"
            && intent.purpose == "create"
            && request
                .password
                .as_ref()
                .is_none_or(|p| p.len() < 8 || p.len() > 4096))
        || ((control.owner() != "idp" || intent.purpose == "join") && request.password.is_some())
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let mut state = control.0.lifecycle.lock().await;
    if control.0.shutdown.is_cancelled() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if state
        .intent
        .as_ref()
        .is_some_and(|retained| retained != &intent)
    {
        return StatusCode::CONFLICT.into_response();
    }
    if let Some(active) = state.active.as_ref() {
        return if active.is_cancelled() {
            StatusCode::CONFLICT
        } else {
            StatusCode::ACCEPTED
        }
        .into_response();
    }
    if let Err(error) = control.0.admission.bind(&headers, &canonical) {
        return error.into_response();
    }
    state.intent = Some(intent.clone());
    let cancellation = control.0.shutdown.child_token();
    state.active = Some(cancellation.clone());
    state.view = if intent.purpose == "join" {
        "joining"
    } else {
        "creating"
    };
    if control
        .0
        .commands
        .try_send(IndependentCreateCommand {
            intent,
            password: request.password,
            cancellation,
            headers,
        })
        .is_err()
    {
        state.active = None;
        state.view = "unfinished";
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    StatusCode::ACCEPTED.into_response()
}
#[utoipa::path(post, tag = "default", path = "/setup/intent", responses((status = 200, body = IndependentCreateIntent, description = "Exact retained nonsecret operation with authenticated issuer"), (status = 204, description = "No bound operation; empty body"), (status = 401, description = "Code invalid or retired"), (status = 503, description = "Admission unavailable")), security(("setup_code" = [])))]
pub(crate) async fn recover(
    State(control): State<IndependentCreateControl>,
    headers: HeaderMap,
) -> Response {
    match control.0.admission.bound_intent(&headers) {
        Ok(Some(intent)) => match serde_json::from_str::<IndependentCreateIntent>(&intent) {
            Ok(intent) => Json(intent).into_response(),
            Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        },
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error.into_response(),
    }
}
#[utoipa::path(post, tag = "default", path = "/setup/cancel", request_body = IndependentCreateIntent, responses((status = 204, description = "Cancellation signaled; not cleanup completion"), (status = 401, description = "Code invalid or retired"), (status = 409, description = "No matching active operation"), (status = 503, description = "Admission unavailable")), security(("setup_code" = [])))]
pub(crate) async fn cancel(
    State(control): State<IndependentCreateControl>,
    headers: HeaderMap,
    Json(intent): Json<IndependentCreateIntent>,
) -> Response {
    if let Err(error) = control.0.admission.check(&headers) {
        return error.into_response();
    }
    let mut state = control.0.lifecycle.lock().await;
    if state.intent.as_ref() != Some(&intent) {
        return StatusCode::CONFLICT.into_response();
    }
    let Some(active) = state.active.as_ref() else {
        return StatusCode::CONFLICT.into_response();
    };
    active.cancel();
    state.challenges.clear();
    state.history = None;
    state.view = "unfinished";
    StatusCode::NO_CONTENT.into_response()
}
#[utoipa::path(get, tag = "default", path = "/setup/status", responses((status = 200, body = crate::independent_openapi::IndependentSetupStatus, description = "Owner-local state; no login Client or secret"), (status = 503, description = "Admission unavailable (empty body), or current owner readiness failed (nonsecret status with ready:false)")))]
pub(crate) async fn status(State(control): State<IndependentCreateControl>) -> Response {
    if control.0.admission.health().is_err() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let view = control.0.lifecycle.lock().await.view;
    if view == "ready" && control.check_readiness().await.is_err() {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"stage":"installation", "state":"unavailable", "ready":false, "owners":[control.owner()]}))).into_response();
    }
    Json(serde_json::json!({"stage":"installation", "state":view, "ready":view == "ready", "owners":[control.owner()]})).into_response()
}
async fn normal(State(control): State<IndependentCreateControl>, request: Request) -> Response {
    let router = control
        .0
        .normal
        .read()
        .ok()
        .and_then(|normal| normal.clone());
    match router {
        Some(router) => router
            .oneshot(request)
            .await
            .expect("owner router is infallible"),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
