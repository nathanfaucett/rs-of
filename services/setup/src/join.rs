use core::result::Result as StdResult;

use std::{
    fmt, fs,
    io::{self, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use iroh_base::{EndpointId, SecretKey};
use of_device_client::{
    apis::{
        Error as DeviceError,
        configuration::Configuration as DeviceConfiguration,
        default_api::{
            ApprovePairingParams, PairingApprovalPayloadParams, RequestPairingOnceParams,
            approve_pairing, list_devices, pairing_approval_payload, request_pairing_once,
        },
    },
    models::{DeviceInfo, DevicePairingApprovalRequest, DevicePairingRequest, DeviceState},
};
use of_idp_client::{
    apis::{
        configuration::Configuration,
        default_api::{TokenParams, token},
    },
    models::SubjectTokenType,
};
use of_model::{device::contract::device_pairing::device_pairing_approval_payload, id::Id};
use reqwest::{Client, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{net::TcpListener, sync::Mutex, task::spawn_blocking};
use tower_http::cors::CorsLayer;

use crate::{
    IndependentOwnerPublic, LocalTls, LocalTlsConfig, SetupAdmission, SetupAdmissionError,
    absolute, create_private_directory, present, private_options, random_capability, read_private,
    require_private, setup_admission_middleware, setup_origin, sync_ancestry,
};

const PATH: &str = "/setup/join";
const STORE: &str = "join-v1.json";

/// Private, synchronous storage for one Join journal.
///
/// The opaque bytes contain the endpoint secret key. Store them without changes.
pub trait JoinJournalStore: Send + Sync {
    /// Return `None` only when no journal has been committed.
    ///
    /// # Errors
    /// Return an error if the committed state cannot be read.
    fn load(&self) -> io::Result<Option<Vec<u8>>>;

    /// Atomically replace the journal. Return success only after durable commit.
    ///
    /// # Errors
    /// Return an error if durable commit cannot be confirmed.
    fn save(&self, journal: &[u8]) -> io::Result<()>;
}

/// Protected local Device join, served over native or verified proxy HTTPS.
pub struct JoinHost {
    listener: Option<(SocketAddr, LocalTls)>,
    origin: HeaderValue,
    state: Arc<Host>,
}

impl fmt::Debug for JoinHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinHost").finish_non_exhaustive()
    }
}

struct TlsConfig(PathBuf, SocketAddr);

impl LocalTlsConfig for TlsConfig {
    fn tls_directory(&self) -> PathBuf {
        self.0.join("tls")
    }

    fn tls_address(&self) -> SocketAddr {
        self.1
    }
}

impl JoinHost {
    /// # Errors
    /// Fails if the origin or address is invalid, the root exists, or protected setup cannot start.
    pub fn initialize(root: &Path, address: SocketAddr, origin: &str) -> io::Result<()> {
        let binding = host_binding(address, origin)?;
        create_private_directory(root, false)?;
        let tls = TlsConfig(root.to_owned(), address);
        LocalTls::create(&tls)?;
        SetupAdmission::open(&root.join("admission"))?;
        let mut file = private_options(true).open(root.join("binding"))?;
        file.write_all(binding.as_bytes())?;
        file.sync_all()?;
        sync_ancestry(root)
    }

    /// # Errors
    /// Fails on a changed host binding, missing state, or invalid protected material.
    pub fn open(root: &Path, address: SocketAddr, origin: &str) -> io::Result<Self> {
        let binding = host_binding(address, origin)?;
        let root = absolute(root)?;
        require_private(&root, true)?;
        if read_private(&root.join("binding"))? != binding.as_bytes() {
            return Err(io::Error::other(
                "join host address or browser origin changed",
            ));
        }
        require_private(&root.join("admission"), true)?;
        let admission = Arc::new(SetupAdmission::open(&root.join("admission"))?);
        let tls = LocalTls::open(&TlsConfig(root.clone(), address))?;
        Self::with_store(
            origin,
            Store::File {
                root,
                admission,
                node_admission: None,
            },
            Some((address, tls)),
        )
    }

    /// Open the protected join flow for a caller-owned, verified HTTPS route.
    /// POST uses Node admission while retaining the private Join admission state.
    /// The caller must lock the root and retain the exact public and browser origins.
    ///
    /// # Errors
    /// Fails when the origin or protected retained evidence is invalid.
    pub fn open_for_proxy(
        root: &Path,
        origin: &str,
        node_admission: Arc<SetupAdmission>,
    ) -> io::Result<Self> {
        if !origin.starts_with("https://") {
            return Err(io::Error::other("join requires an HTTPS browser origin"));
        }
        let root = absolute(root)?;
        create_private_directory(&root, true)?;
        require_private(&root, true)?;
        let path = root.join("admission");
        if present(&root.join(STORE))? {
            require_private(&path, true)?;
        }
        let admission = Arc::new(SetupAdmission::open(&path)?);
        Self::with_store(
            origin,
            Store::File {
                root,
                admission,
                node_admission: Some(node_admission),
            },
            None,
        )
    }

    /// Open Join on a caller-owned, verified HTTPS route without Join files or a
    /// Join code. POST checks the Node admission; Join does not bind or retire it.
    /// The caller must retain the exact public and browser origins and ensure only
    /// one live Join host owns this journal, including across processes.
    ///
    /// # Errors
    /// Fails when the origin or retained journal is invalid or unavailable.
    pub fn open_for_proxy_with_journal(
        origin: &str,
        node_admission: Arc<SetupAdmission>,
        journal: Arc<dyn JoinJournalStore>,
    ) -> io::Result<Self> {
        if !origin.starts_with("https://") {
            return Err(io::Error::other("join requires an HTTPS browser origin"));
        }
        Self::with_store(
            origin,
            Store::Journal {
                journal,
                node_admission,
            },
            None,
        )
    }

    fn with_store(
        origin: &str,
        store: Store,
        listener: Option<(SocketAddr, LocalTls)>,
    ) -> io::Result<Self> {
        let state = Arc::new(Host {
            store,
            operation: Mutex::new(()),
        });
        state.load().map_err(|_error| {
            io::Error::other("join state needs repair; retained data was not replaced")
        })?;
        Ok(Self {
            listener,
            origin: setup_origin(origin)?,
            state,
        })
    }

    /// # Errors
    /// Fails if the loopback HTTPS listener cannot bind or serve.
    pub async fn serve(self) -> io::Result<()> {
        let (address, tls) = self
            .listener
            .as_ref()
            .ok_or_else(|| io::Error::other("proxy join requires caller-owned serving"))?;
        let listener = TcpListener::bind(address).await?;
        axum::serve(tls.listener(listener), self.router()).await
    }

    /// Serve the same protected operations through either transport.
    pub fn router(&self) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(self.origin.clone())
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
            .allow_private_network(true);
        let writes = Router::new().route(PATH, post(action));
        let writes = match &self.state.store {
            Store::File {
                admission,
                node_admission: Some(node),
                ..
            } => writes.route_layer(middleware::from_fn_with_state(
                (Arc::clone(node), Arc::clone(admission)),
                proxy_admission,
            )),
            store => writes.route_layer(middleware::from_fn_with_state(
                Arc::clone(store.admission()),
                setup_admission_middleware,
            )),
        };
        Router::new()
            .route(PATH, get(status))
            .merge(writes)
            .with_state(Arc::clone(&self.state))
            .route_layer(middleware::from_fn_with_state(
                self.origin.clone(),
                require_origin,
            ))
            .layer(DefaultBodyLimit::max(32 * 1024))
            .layer(cors)
            .layer(middleware::from_fn(no_store))
    }

    /// Returns the selected service bases only after verified approval and retirement.
    ///
    /// # Errors
    /// Fails when retained approval evidence cannot be read or does not match.
    pub async fn approved_bases(&self) -> io::Result<Option<(String, String)>> {
        let _guard = self.state.operation.lock().await;
        let Some(mut journal) = self
            .state
            .completed()
            .map_err(|_error| io::Error::other("retained join evidence unavailable"))?
        else {
            return Ok(None);
        };
        if !journal.storage_verified {
            verify_installation(
                &remote_client().map_err(|_| io::Error::other("cannot open Join client"))?,
                &journal.intent,
            )
            .await
            .map_err(|_| io::Error::other("retained Join Installation evidence unavailable"))?;
            journal.storage_verified = true;
            self.state
                .save(&journal)
                .map_err(|_| io::Error::other("cannot save Join Installation evidence"))?;
        }
        Ok(Some((journal.intent.issuer, journal.intent.storage)))
    }
}

fn host_binding(address: SocketAddr, origin: &str) -> io::Result<String> {
    setup_origin(origin)?;
    if !origin.starts_with("https://") {
        return Err(io::Error::other("join requires an HTTPS browser origin"));
    }
    Ok(format!("{}\n{origin}", LocalTls::api_base(address)?))
}

async fn require_origin(
    State(allowed): State<HeaderValue>,
    request: Request,
    next: Next,
) -> Response {
    let mut origins = request.headers().get_all(header::ORIGIN).iter();
    if origins.next() != Some(&allowed) || origins.next().is_some() {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

async fn proxy_admission(
    State((node, join)): State<(Arc<SetupAdmission>, Arc<SetupAdmission>)>,
    mut request: Request,
    next: Next,
) -> StdResult<Response, SetupAdmissionError> {
    let headers = request.headers().clone();
    let mut value = spawn_blocking(move || {
        node.check(&headers)?;
        let code = join.current_code()?.ok_or(SetupAdmissionError::Denied)?;
        HeaderValue::try_from(format!("Setup {code}"))
            .map_err(|_error| SetupAdmissionError::Unavailable)
    })
    .await
    .map_err(|_error| SetupAdmissionError::Unavailable)??;
    value.set_sensitive(true);
    request.headers_mut().insert(header::AUTHORIZATION, value);
    Ok(next.run(request).await)
}

async fn no_store(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

struct Host {
    store: Store,
    operation: Mutex<()>,
}

enum Store {
    File {
        root: PathBuf,
        admission: Arc<SetupAdmission>,
        node_admission: Option<Arc<SetupAdmission>>,
    },
    Journal {
        journal: Arc<dyn JoinJournalStore>,
        node_admission: Arc<SetupAdmission>,
    },
}

impl Store {
    const fn admission(&self) -> &Arc<SetupAdmission> {
        match self {
            Self::File { admission, .. } => admission,
            Self::Journal { node_admission, .. } => node_admission,
        }
    }

    fn retired(&self, journal: &Journal) -> Result<bool> {
        match self {
            Self::File { admission, .. } => Ok(admission.is_retired()?),
            Self::Journal { .. } => Ok(matches!(journal.phase, Phase::Approved(_))),
        }
    }

    // For a DB journal, the prepared commit binds the intent and the approved commit retires it.
    fn bind(&self, headers: &HeaderMap, journal: &Journal) -> Result<()> {
        if let Self::File { admission, .. } = self {
            admission.bind(headers, &journal.binding()?)?;
        }
        Ok(())
    }

    fn retire(&self, headers: &HeaderMap, journal: &Journal) -> Result<()> {
        if let Self::File { admission, .. } = self {
            admission.retire(headers, &journal.binding()?)?;
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Intent {
    issuer: String,
    device: String,
    storage: String,
    client_id: String,
    application: String,
    accepting_public_key: String,
    name: String,
    address: String,
}

impl Intent {
    fn valid(&self) -> bool {
        fn base(raw: &str) -> bool {
            bounded(raw)
                && Url::parse(raw).is_ok_and(|url| {
                    url.scheme() == "https"
                        && url.host_str().is_some()
                        && url.username().is_empty()
                        && url.password().is_none()
                        && url.query().is_none()
                        && url.fragment().is_none()
                        && url.as_str().trim_end_matches('/') == raw
                })
        }
        [
            self.issuer.as_str(),
            self.device.as_str(),
            self.storage.as_str(),
        ]
        .into_iter()
        .all(base)
            && self.issuer != self.device
            && self.issuer != self.storage
            && self.device != self.storage
            && bounded(&self.client_id)
            && bounded(&self.application)
            && bounded(&self.name)
            && bounded(&self.address)
            && endpoint(&self.accepting_public_key)
    }
}

fn bounded(text: &str) -> bool {
    text.len() <= 512 && !text.trim().is_empty() && !text.chars().any(char::is_control)
}

fn endpoint(value: &str) -> bool {
    value
        .parse::<EndpointId>()
        .is_ok_and(|id| id.to_string() == value)
}

fn record_id(value: &str) -> bool {
    value
        .parse::<Id>()
        .is_ok_and(|id| !id.is_nil() && id.to_string() == value)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    intent: Intent,
    key: [u8; 32],
    public_key: String,
    phase: Phase,
    #[serde(default)]
    storage_verified: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Prepared,
    Requesting,
    Dispatching,
    Pending(String),
    Approving(String),
    Approved(String),
}

impl Phase {
    fn id(&self) -> Option<&str> {
        match self {
            Self::Pending(id) | Self::Approving(id) | Self::Approved(id) => Some(id),
            _ => None,
        }
    }
}

impl Journal {
    fn prepared(intent: Intent) -> Self {
        let key = SecretKey::generate();
        Self {
            intent,
            key: key.to_bytes(),
            public_key: key.public().to_string(),
            phase: Phase::Prepared,
            storage_verified: false,
        }
    }

    fn binding(&self) -> Result<String> {
        let bytes = serde_json::to_vec(&(&self.intent, &self.public_key))
            .map_err(|_error| Failure::Unavailable)?;
        let hash = Sha256::digest(bytes);
        Ok(format!("join-v1:{}", URL_SAFE_NO_PAD.encode(hash)))
    }

    fn matches(&self, device: &DeviceInfo) -> bool {
        device.public_key == self.public_key
            && device.name == self.intent.name
            && device.address == self.intent.address
            && record_id(&device.id)
            && self.phase.id().is_none_or(|id| id == device.id)
            && device.revoked_at.flatten().is_none()
    }
}

#[derive(Clone, Copy)]
enum Failure {
    Admission(SetupAdmissionError),
    Invalid,
    Conflict,
    Unavailable,
    Unauthorized,
    Remote,
    Uncertain,
}

type Result<T> = StdResult<T, Failure>;

impl From<SetupAdmissionError> for Failure {
    fn from(value: SetupAdmissionError) -> Self {
        Self::Admission(value)
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Admission(error) => return error.into_response(),
            Self::Invalid => (StatusCode::BAD_REQUEST, "Invalid join settings."),
            Self::Conflict => (
                StatusCode::CONFLICT,
                "Join evidence disagrees. Retained state was not replaced.",
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Retained join state is unavailable.",
            ),
            Self::Unauthorized => (
                StatusCode::FORBIDDEN,
                "Use an authorized Installation User who can read and enroll Devices and owns the accepting Device record.",
            ),
            Self::Remote => (
                StatusCode::BAD_GATEWAY,
                "The selected Installation could not complete this operation. Retry or sign in again.",
            ),
            Self::Uncertain => (
                StatusCode::CONFLICT,
                "Pairing request outcome is unknown. Request pairing again to retry the same enrollment.",
            ),
        };
        (status, message).into_response()
    }
}

#[derive(Deserialize)]
#[serde(
    tag = "action",
    deny_unknown_fields,
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum Action {
    Prepare { intent: Intent },
    Resume,
    Request { access_token: String },
    Approve { access_token: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View<'a> {
    stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    intent: Option<&'a Intent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    public_key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_id: Option<&'a str>,
}

fn view(journal: Option<&Journal>) -> Response {
    let stage = match journal.map(|value| &value.phase) {
        None => "setup",
        Some(Phase::Prepared | Phase::Requesting | Phase::Dispatching) => "prepared",
        Some(Phase::Pending(_) | Phase::Approving(_)) => "pending",
        Some(Phase::Approved(_)) => "approved",
    };
    Json(View {
        stage,
        intent: journal
            .filter(|_| stage != "approved")
            .map(|item| &item.intent),
        public_key: journal.map(|item| item.public_key.as_str()),
        device_id: journal.and_then(|item| item.phase.id()),
    })
    .into_response()
}

impl Host {
    fn completed(&self) -> Result<Option<Journal>> {
        match self.load()? {
            Some(journal) if self.store.retired(&journal)? => Ok(Some(journal)),
            _ => Ok(None),
        }
    }

    fn load(&self) -> Result<Option<Journal>> {
        let (bytes, file_admission) = match &self.store {
            Store::File {
                root, admission, ..
            } => {
                let bound = admission.host_intent()?;
                let retired = admission.is_retired()?;
                let path = root.join(STORE);
                if !present(&path).map_err(|_error| Failure::Unavailable)? {
                    return if bound.is_none() && !retired {
                        Ok(None)
                    } else {
                        Err(Failure::Unavailable)
                    };
                }
                let bytes = read_private(&path).map_err(|_error| Failure::Unavailable)?;
                (bytes, Some((bound, retired)))
            }
            Store::Journal { journal, .. } => {
                let Some(bytes) = journal.load().map_err(|_error| Failure::Unavailable)? else {
                    return Ok(None);
                };
                (bytes, None)
            }
        };
        let journal: Journal =
            serde_json::from_slice(&bytes).map_err(|_error| Failure::Unavailable)?;
        if !journal.intent.valid()
            || SecretKey::from_bytes(&journal.key).public().to_string() != journal.public_key
            || journal.public_key == journal.intent.accepting_public_key
            || journal.phase.id().is_some_and(|id| !record_id(id))
            || match file_admission {
                Some((bound, retired)) => {
                    (retired && !matches!(journal.phase, Phase::Approved(_)))
                        || match bound {
                            Some(value) => value != journal.binding()?,
                            None => !matches!(journal.phase, Phase::Prepared),
                        }
                }
                None => {
                    matches!(journal.phase, Phase::Requesting)
                        || matches!(journal.phase, Phase::Approved(_)) != journal.storage_verified
                }
            }
        {
            return Err(Failure::Conflict);
        }
        Ok(Some(journal))
    }

    fn save(&self, journal: &Journal) -> Result<()> {
        let write = || -> io::Result<()> {
            let root = match &self.store {
                Store::File { root, .. } => root,
                Store::Journal { journal: store, .. } => {
                    return store.save(&serde_json::to_vec(journal)?);
                }
            };
            let path = root.join(format!(".join-{}", random_capability()?));
            let mut file = private_options(true).open(&path)?;
            serde_json::to_writer(&mut file, journal)?;
            file.sync_all()?;
            fs::rename(path, root.join(STORE))?;
            sync_ancestry(root)
        };
        write().map_err(|_error| Failure::Unavailable)
    }

    fn prepare(
        &self,
        headers: &HeaderMap,
        retained: Option<Journal>,
        intent: Intent,
    ) -> Result<Journal> {
        if !intent.valid() {
            return Err(Failure::Invalid);
        }
        let journal = match retained {
            Some(journal) if journal.intent == intent => journal,
            Some(_) => return Err(Failure::Conflict),
            None => {
                let journal = Journal::prepared(intent);
                self.save(&journal)?;
                journal
            }
        };
        self.store.bind(headers, &journal)?;
        Ok(journal)
    }
}

async fn status(State(host): State<Arc<Host>>) -> Result<Response> {
    let _guard = host.operation.lock().await;
    Ok(view(host.completed()?.as_ref()))
}

async fn action(
    State(host): State<Arc<Host>>,
    headers: HeaderMap,
    Json(action): Json<Action>,
) -> Result<Response> {
    let _guard = host.operation.lock().await;
    host.store.admission().check(&headers)?;
    let retained = host.load()?;
    if matches!(host.store, Store::Journal { .. })
        && retained
            .as_ref()
            .is_some_and(|item| matches!(item.phase, Phase::Approved(_)))
    {
        return Err(SetupAdmissionError::Denied.into());
    }
    let action = match action {
        Action::Prepare { intent } => {
            let journal = host.prepare(&headers, retained, intent)?;
            if matches!(journal.phase, Phase::Approved(_)) {
                host.finish(&headers, journal).await?;
                return Ok(view(host.load()?.as_ref()));
            }
            return Ok(view(Some(&journal)));
        }
        other => other,
    };
    let mut journal = retained.ok_or(Failure::Conflict)?;
    host.store.bind(&headers, &journal)?;
    if let Phase::Approved(_) = journal.phase {
        host.finish(&headers, journal).await?;
        return Ok(view(host.load()?.as_ref()));
    }
    match action {
        Action::Resume => (),
        Action::Request { access_token } => {
            let device = device_client(&journal.intent, access_token).await?;
            host.request(&device, &mut journal).await?;
        }
        Action::Approve { access_token } => {
            let device = device_client(&journal.intent, access_token).await?;
            host.approve(&device, &mut journal).await?;
            host.store.retire(&headers, &journal)?;
        }
        Action::Prepare { .. } => unreachable!("prepare returns before owner calls"),
    }
    Ok(view(Some(&journal)))
}

#[derive(Deserialize, PartialEq, Eq)]
struct InstallationBases {
    issuer: String,
    device: String,
    storage: String,
}

fn remote_client() -> Result<Client> {
    Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| Failure::Remote)
}

async fn public_evidence(client: &Client, base: &str, path: &str) -> Result<reqwest::Response> {
    client
        .get(format!("{base}{path}"))
        .send()
        .await
        .map_err(|_| Failure::Remote)
}

async fn verify_installation(client: &Client, intent: &Intent) -> Result<()> {
    const OWNER: &str = "/setup/owner/public";
    const FIXTURE: &str = "/setup/seeded-installation";
    let public = public_evidence(client, &intent.issuer, OWNER).await?;
    if public.status() == StatusCode::NOT_FOUND {
        let expected = InstallationBases {
            issuer: intent.issuer.clone(),
            device: intent.device.clone(),
            storage: intent.storage.clone(),
        };
        for base in [&intent.issuer, &intent.storage] {
            let response = public_evidence(client, base, FIXTURE).await?;
            if response.status() != StatusCode::OK
                || response
                    .json::<InstallationBases>()
                    .await
                    .map_err(|_| Failure::Remote)?
                    != expected
            {
                return Err(Failure::Conflict);
            }
        }
        return Ok(());
    }
    async fn owner(response: reqwest::Response) -> Result<IndependentOwnerPublic> {
        if response.status() != StatusCode::OK {
            return Err(Failure::Remote);
        }
        response.json().await.map_err(|_| Failure::Remote)
    }
    let idp = owner(public).await?;
    let exact = &idp.operation;
    if idp.owner != "idp"
        || exact.canonical().is_err()
        || exact.issuer != intent.issuer
        || exact.device != intent.device
        || exact.storage != intent.storage
    {
        return Err(Failure::Conflict);
    }
    let storage = owner(public_evidence(client, &intent.storage, OWNER).await?).await?;
    if storage.owner != "storage" || storage.operation != *exact {
        return Err(Failure::Conflict);
    }
    Ok(())
}

async fn device_client(intent: &Intent, access_token: String) -> Result<DeviceConfiguration> {
    if access_token.is_empty() || access_token.len() > 16_384 {
        return Err(Failure::Unauthorized);
    }
    let client = remote_client()?;
    verify_installation(&client, intent).await?;
    let idp = Configuration {
        client: client.clone(),
        base_path: intent.issuer.clone(),
        ..Configuration::default()
    };
    let result = token(
        &idp,
        TokenParams {
            client_id: Some(intent.client_id.clone()),
            grant_type: Some("urn:ietf:params:oauth:grant-type:token-exchange".into()),
            resource: Some(intent.device.clone()),
            subject_token: Some(access_token),
            subject_token_type: Some(
                SubjectTokenType::UrnColonIetfColonParamsColonOauthColonTokenTypeColonAccessToken,
            ),
            ..TokenParams::default()
        },
    )
    .await
    .map_err(|_error| Failure::Remote)?;
    if result.access_token.is_empty() {
        return Err(Failure::Unauthorized);
    }
    Ok(DeviceConfiguration {
        client,
        base_path: intent.device.clone(),
        bearer_access_token: Some(result.access_token),
        ..DeviceConfiguration::default()
    })
}

async fn observe(api: &DeviceConfiguration, journal: &Journal) -> Result<Option<DeviceInfo>> {
    let rows = list_devices(api).await.map_err(|error| match error {
        DeviceError::ResponseError(response) if matches!(response.status.as_u16(), 401 | 403) => {
            Failure::Unauthorized
        }
        _ => Failure::Remote,
    })?;
    // The current owner API can recover the record only in the accepting Device's legacy owner view.
    if !rows.iter().any(|row| {
        row.public_key == journal.intent.accepting_public_key
            && row.state == DeviceState::Approved
            && row.revoked_at.flatten().is_none()
    }) {
        return Err(Failure::Unauthorized);
    }
    let mut matches = rows
        .into_iter()
        .filter(|row| row.public_key == journal.public_key);
    let record = matches.next();
    if matches.next().is_some() || record.as_ref().is_some_and(|row| !journal.matches(row)) {
        return Err(Failure::Conflict);
    }
    Ok(record)
}

impl Host {
    async fn finish(&self, headers: &HeaderMap, mut journal: Journal) -> Result<()> {
        if !journal.storage_verified {
            verify_installation(&remote_client()?, &journal.intent).await?;
            journal.storage_verified = true;
            self.save(&journal)?;
        }
        self.store.retire(headers, &journal)?;
        Ok(())
    }

    async fn request(&self, api: &DeviceConfiguration, journal: &mut Journal) -> Result<()> {
        if !matches!(
            journal.phase,
            Phase::Prepared | Phase::Requesting | Phase::Dispatching
        ) {
            return Err(Failure::Conflict);
        }
        let record = observe(api, journal).await?;
        let id = if matches!(journal.phase, Phase::Requesting) && record.is_some() {
            // Journals from the legacy POST flow have no requester receipt. Recover by owner read only.
            let record = record.ok_or(Failure::Conflict)?;
            if record.state != DeviceState::Pending {
                return Err(Failure::Conflict);
            }
            record.id
        } else {
            if matches!(journal.phase, Phase::Prepared) && record.is_some() {
                return Err(Failure::Conflict);
            }
            journal.phase = Phase::Dispatching;
            self.save(journal)?;
            let result = request_pairing_once(
                api,
                RequestPairingOnceParams {
                    device_pairing_request: DevicePairingRequest {
                        name: journal.intent.name.clone(),
                        address: journal.intent.address.clone(),
                        accepting_public_key: journal.intent.accepting_public_key.clone(),
                        public_key: journal.public_key.clone(),
                    },
                },
            )
            .await;
            let result = match result {
                Ok(result) => result,
                Err(DeviceError::ResponseError(response)) => match response.status.as_u16() {
                    400 | 409 => return Err(Failure::Conflict),
                    401 | 403 => return Err(Failure::Unauthorized),
                    404 | 405 => return Err(Failure::Remote),
                    _ => return Err(Failure::Uncertain),
                },
                Err(_) => return Err(Failure::Uncertain),
            };
            if result.state != DeviceState::Pending
                || !record_id(&result.id)
                || record.is_some_and(|row| row.id != result.id)
            {
                return Err(Failure::Conflict);
            }
            result.id
        };
        journal.phase = Phase::Pending(id);
        self.save(journal)
    }

    async fn approve(&self, api: &DeviceConfiguration, journal: &mut Journal) -> Result<()> {
        let (id, retry) = match &journal.phase {
            Phase::Pending(id) => (id.clone(), false),
            Phase::Approving(id) => (id.clone(), true),
            _ => return Err(Failure::Conflict),
        };
        let record = observe(api, journal).await?.ok_or(Failure::Conflict)?;
        if !(retry && record.state == DeviceState::Approved) {
            if record.state != DeviceState::Pending {
                return Err(Failure::Conflict);
            }
            let text = pairing_approval_payload(
                api,
                PairingApprovalPayloadParams {
                    device_id: id.clone(),
                },
            )
            .await
            .map_err(|_error| Failure::Remote)?
            .payload;
            let expected = device_pairing_approval_payload(
                id.parse().map_err(|_error| Failure::Conflict)?,
                &record.name,
                &record.public_key,
                &record.address,
            );
            if text != expected {
                return Err(Failure::Conflict);
            }
            let key = SecretKey::from_bytes(&journal.key);
            let signature = URL_SAFE_NO_PAD.encode(key.sign(text.as_bytes()).to_bytes());
            journal.phase = Phase::Approving(id.clone());
            self.save(journal)?;
            let record = approve_pairing(
                api,
                ApprovePairingParams {
                    device_id: id.clone(),
                    device_pairing_approval_request: DevicePairingApprovalRequest { signature },
                },
            )
            .await
            .map_err(|_error| Failure::Remote)?;
            if record.state != DeviceState::Approved || !journal.matches(&record) {
                return Err(Failure::Conflict);
            }
        }
        journal.phase = Phase::Approved(id);
        journal.storage_verified = true;
        self.save(journal)
    }
}
