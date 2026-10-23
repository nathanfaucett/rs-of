use std::{
    collections::BTreeMap,
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use api::SecurityAddon;
use api::{IndependentPeer, LocalTls};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use db::Database;
use device_service::DeviceIdentity;
use device_service::DeviceRepo;
use device_service::HostedControlPlane;
use device_service::independent_join::IndependentJoinApproval;
use device_service::independent_join::IndependentJoinRequest;
use device_service::join_history::CheckJoinHistory;
use device_service::join_history::JoinHistoryContext;
use device_service::join_history::JoinHistorySource;
use device_service::replica::DbDeviceRepo;
use device_service::replica::DbSelectionPolicyRepo;
use device_service::replica::SelectionPolicy;
use idp_model::{
    contract::{
        IdentityAction, IdentityResource, PermissionEvaluationRequest, PermissionSubject,
        PermissionTarget,
    },
    model::Id,
};
use iroh_chain::{EndpointIdStore, PairingOffer, Server};
use management_service::LocalPermissions;
use model::contract::PrincipalType;
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_util::sync::CancellationToken;

pub struct IndependentDeviceRuntime {
    router: Router,
    protocol: iroh::protocol::Router,
    source: Arc<IndependentJoinSource>,
    task: JoinHandle<io::Result<()>>,
}
impl IndependentDeviceRuntime {
    pub fn router(&self) -> Router {
        self.router.clone()
    }
    pub async fn shutdown(self) -> io::Result<()> {
        self.source.cancellation.cancel();
        self.task.await.map_err(io::Error::other)??;
        self.protocol.shutdown().await.map_err(io::Error::other)
    }
}

pub async fn build_independent_device(
    config: &crate::AppConfig,
    engine: Arc<Database>,
    plane: Arc<HostedControlPlane>,
    identity: Arc<DeviceIdentity>,
    issuer: &str,
    idp: &str,
    device: &str,
    storage: &str,
    cancellation: CancellationToken,
) -> io::Result<IndependentDeviceRuntime> {
    let origin = config
        .server
        .setup_ui_origin
        .as_deref()
        .unwrap_or("https://localhost:5173")
        .parse::<axum::http::HeaderValue>()
        .map_err(io::Error::other)?;
    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(origin)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ])
        .allow_private_network(true);
    let plane = Arc::new(
        plane
            .as_ref()
            .clone()
            .with_local_devices(Arc::new(DbDeviceRepo::new(engine.clone()))),
    );
    let server = Server::new(identity.endpoint(), EndpointIdStore::default());
    let (approve_sender, approval_jobs) = tokio::sync::mpsc::channel(1);
    let identity_engine = crate::provisioning::open_identity_database(config)?;
    let source = Arc::new(IndependentJoinSource {
        identity_engine: identity_engine.clone(),
        engine: engine.clone(),
        plane: plane.clone(),
        identity,
        issuer: issuer.into(),
        idp: idp.into(),
        device: device.into(),
        server: server.clone(),
        offers: Mutex::new(BTreeMap::new()),
        answered: Mutex::new(BTreeMap::new()),
        approve: approve_sender,
        acceptance_gate: Mutex::new(()),
        cancellation,
    });
    let receiver = server.subscribe_pairing_offers();
    let protocol = server.router(DenyData);
    let running = source.clone();
    let task = tokio::spawn(async move { running.run(receiver, approval_jobs).await });
    let current = source.clone();
    let check: CheckJoinHistory = Arc::new(move |approval| {
        let current = current.clone();
        Box::pin(async move {
            current
                .verify(&approval, IdentityAction::ManagementSecuritySync)
                .await
        })
    });
    let history = JoinHistorySource::new(
        engine.clone(),
        JoinHistoryContext::Device,
        std::path::Path::new(&config.data_dir).join("join-source-history"),
        check,
        source.cancellation.child_token(),
    );
    let routes = join_routes()
        .with_state(source.clone())
        .split_for_parts()
        .0
        .merge(history.router())
        .layer(cors);
    let routes = if config.server.prefix().is_empty() {
        routes
    } else {
        Router::new().nest(config.server.prefix(), routes)
    };
    let router = crate::build_router(
        identity_engine,
        engine,
        device,
        config.server.prefix(),
        storage,
        plane,
        None,
    )
    .await
    .map_err(io::Error::other)?
    .merge(routes);
    Ok(IndependentDeviceRuntime {
        router,
        protocol,
        source,
        task,
    })
}

#[derive(Debug)]
struct DenyData;
impl iroh::protocol::ProtocolHandler for DenyData {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        connection.close(1u32.into(), b"Management does not serve resource DATA");
        Ok(())
    }
}
struct QueuedOffer {
    offer: PairingOffer,
    accepted: AtomicBool,
}
struct ApprovalWork {
    id: Id,
    headers: HeaderMap,
}
struct IndependentJoinSource {
    identity_engine: Arc<Database>,
    engine: Arc<Database>,
    plane: Arc<HostedControlPlane>,
    identity: Arc<DeviceIdentity>,
    issuer: String,
    idp: String,
    device: String,
    server: Server,
    offers: Mutex<BTreeMap<Id, Arc<QueuedOffer>>>,
    answered: Mutex<BTreeMap<Id, PairingOffer>>,
    approve: tokio::sync::mpsc::Sender<ApprovalWork>,
    acceptance_gate: Mutex<()>,
    cancellation: CancellationToken,
}
impl IndependentJoinSource {
    async fn run(
        &self,
        mut offers: tokio::sync::broadcast::Receiver<PairingOffer>,
        mut approvals: tokio::sync::mpsc::Receiver<ApprovalWork>,
    ) -> io::Result<()> {
        loop {
            let offer = tokio::select! { biased; () = self.cancellation.cancelled() => break, Some(job) = approvals.recv() => {
                let id=job.id;
                let result=tokio::select! { biased; ()=self.cancellation.cancelled() => break, result=approve_work(self, job) => result };
                if let Err(status)=result { if let Some(queued)=self.offers.lock().await.get(&id) { queued.accepted.store(false,Ordering::SeqCst); } log::warn!("Independent source Join approval unfinished status_code={}",status.as_u16()); }
                continue;
            }, offer = offers.recv() => match offer { Ok(offer) => offer, Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue, Err(_) => break } };
            let valid = serde_json::from_slice::<IndependentJoinRequest>(&offer.payload).is_ok_and(
                |request| {
                    self.check_request(&request).is_ok()
                        && request.endpoint_id == offer.remote_id.to_string()
                },
            );
            if valid && self.current(&offer) {
                self.offers.lock().await.insert(
                    Id::now_v7(),
                    Arc::new(QueuedOffer {
                        offer,
                        accepted: AtomicBool::new(false),
                    }),
                );
            }
        }
        self.offers.lock().await.clear();
        self.answered.lock().await.clear();
        self.server.set_pairing_enabled(false);
        Ok(())
    }
    fn check_request(&self, request: &IndependentJoinRequest) -> io::Result<()> {
        request.validate()?;
        if request.operation.issuer != self.issuer
            || request.source.idp != self.idp
            || request.source.device != self.device
            || request.source.endpoint != self.identity.endpoint_id().to_string()
        {
            return Err(denied());
        }
        Ok(())
    }
    fn current(&self, offer: &PairingOffer) -> bool {
        !self.cancellation.is_cancelled()
            && !offer.is_closed()
            && self.server.pairing_acceptance_generation() == Some(offer.acceptance_generation)
    }
    async fn actor(&self, headers: &HeaderMap, actions: &[IdentityAction]) -> io::Result<Id> {
        let bearer = bearer(headers)?;
        let (claims, _) = self
            .plane
            .validate_actor_token(bearer)
            .await
            .map_err(|_| denied())?;
        if claims.principal_type != PrincipalType::User
            || claims.aud != management_service::MANAGEMENT_APPLICATION_URI
        {
            return Err(denied());
        }
        let owner = claims.sub.parse::<Id>().map_err(|_| denied())?;
        self.plane.active_user(owner).await.map_err(|_| denied())?;
        for action in actions {
            permission(&self.identity_engine, owner, *action).await?;
        }
        Ok(owner)
    }
    async fn verify(
        &self,
        approval: &IndependentJoinApproval,
        action: IdentityAction,
    ) -> io::Result<()> {
        approval.verify()?;
        self.check_request(&approval.request)?;
        let approving = self
            .plane
            .get_storage_endpoint_identity(&self.identity.endpoint_id().to_string())
            .await
            .map_err(|_| denied())?;
        let request = &approval.request;
        let device = DbDeviceRepo::new(self.engine.clone())
            .find_approved_by_public_key(&request.endpoint_id)
            .await
            .map_err(|_| denied())?
            .ok_or_else(denied)?;
        if device.id != request.device_id
            || device.name != request.operation.node
            || device.owner_subject != approval.owner_subject.to_string()
        {
            return Err(denied());
        }
        let policies = DbSelectionPolicyRepo::new(self.engine.clone());
        if !policies
            .get(device.id, &device.owner_subject, Id::nil())
            .await
            .map_err(|_| denied())?
            .is_some_and(|policy| policy.admin_allowed)
        {
            return Err(denied());
        }
        if approving.owner_subject != approval.owner_subject.to_string() {
            self.plane
                .active_user(approval.owner_subject)
                .await
                .map_err(|_| denied())?;
        }
        permission(&self.identity_engine, approval.owner_subject, action).await
    }
}
async fn permission(engine: &Arc<Database>, owner: Id, action: IdentityAction) -> io::Result<()> {
    let resource = match action {
        IdentityAction::ReplicaSignersEnroll => IdentityResource::ReplicaSigner { member_id: None },
        IdentityAction::IdpSecuritySync => IdentityResource::IdpSecurity,
        IdentityAction::ManagementSecuritySync => IdentityResource::ManagementSecurity,
        _ => IdentityResource::DevicePairing,
    };
    let request = PermissionEvaluationRequest {
        request_id: Id::now_v7(),
        subject: PermissionSubject::User { id: owner },
        action,
        target: PermissionTarget::Installation { resource },
    };
    if LocalPermissions::new(engine.clone())
        .evaluate(&request)
        .await
        .map_err(|_| denied())?
    {
        Ok(())
    } else {
        Err(denied())
    }
}
fn bearer(headers: &HeaderMap) -> io::Result<&str> {
    headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or_else(denied)
}
fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "independent Join authorization denied",
    )
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
struct DiscoveryRequest {
    #[schema(min_length = 64, max_length = 64, pattern = "^[0-9a-fA-F]{64}$")]
    nonce: String,
}
#[derive(serde::Deserialize, utoipa::ToSchema)]
struct AcceptanceRequest {
    accepting: bool,
}
#[derive(serde::Deserialize, utoipa::ToSchema)]
struct VerificationRequest {
    approval: IndependentJoinApproval,
    #[schema(value_type = String)]
    context: String,
}

#[utoipa::path(post, path = "/independent/node-joins/discover", operation_id = "discover", security(()), request_body = DiscoveryRequest, responses((status = 200, body = IndependentPeer, description = "Operation-bound Node-join result"), (status = 400, description = "Invalid input or current authorization/operation denied"), (status = 403, description = "Invalid input or current authorization/operation denied"), (status = 503, description = "Invalid input or current authorization/operation denied")))]
async fn discover(
    State(source): State<Arc<IndependentJoinSource>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<IndependentPeer>, StatusCode> {
    if !source.server.pairing_enabled() {
        return Err(StatusCode::FORBIDDEN);
    }
    let input: DiscoveryRequest =
        serde_json::from_value(body).map_err(|_| StatusCode::BAD_REQUEST)?;
    let nonce = input.nonce.as_str();
    if nonce.len() != 64 || !nonce.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    source
        .plane
        .get_storage_endpoint_identity(&source.identity.endpoint_id().to_string())
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let mut peer = IndependentPeer {
        nonce: nonce.into(),
        issuer: source.issuer.clone(),
        idp: source.idp.clone(),
        device: source.device.clone(),
        endpoint: source.identity.endpoint_id().to_string(),
        address: serde_json::to_value(source.identity.endpoint().addr())
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        signature: String::new(),
    };
    peer.signature = source
        .identity
        .sign(
            &peer
                .payload()
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        )
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(Json(peer))
}
#[utoipa::path(get, path = "/independent/node-joins/acceptance", operation_id = "accepting", security(("authorization" = [])), responses((status = 200, body = AcceptanceRequest, description = "Operation-bound Node-join result"), (status = 403, description = "Invalid input or current authorization/operation denied")))]
async fn accepting(
    State(source): State<Arc<IndependentJoinSource>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    source
        .actor(
            &headers,
            &[
                IdentityAction::DevicePairingRead,
                IdentityAction::ReplicaSignersEnroll,
            ],
        )
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    Ok(Json(
        serde_json::json!({"accepting":source.server.pairing_enabled()}),
    ))
}
#[utoipa::path(put, path = "/independent/node-joins/acceptance", operation_id = "set_accepting", security(("authorization" = [])), request_body = AcceptanceRequest, responses((status = 204, description = "Operation-bound Node-join result"), (status = 400, description = "Invalid input or current authorization/operation denied"), (status = 403, description = "Invalid input or current authorization/operation denied")))]
async fn set_accepting(
    State(source): State<Arc<IndependentJoinSource>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<StatusCode, StatusCode> {
    source
        .actor(
            &headers,
            &[
                IdentityAction::DevicePairingUpdate,
                IdentityAction::ReplicaSignersEnroll,
            ],
        )
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let input: AcceptanceRequest =
        serde_json::from_value(body).map_err(|_| StatusCode::BAD_REQUEST)?;
    let enabled = input.accepting;
    let _acceptance = source.acceptance_gate.lock().await;
    source.server.set_pairing_enabled(enabled);
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get, path = "/independent/node-joins/pending", operation_id = "pending", security(("authorization" = [])), responses((status = 200, body = Vec<serde_json::Value>, description = "Operation-bound Node-join result"), (status = 403, description = "Invalid input or current authorization/operation denied"), (status = 409, description = "Invalid input or current authorization/operation denied")))]
async fn pending(
    State(source): State<Arc<IndependentJoinSource>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    source
        .actor(
            &headers,
            &[
                IdentityAction::DevicePairingRead,
                IdentityAction::ReplicaSignersEnroll,
            ],
        )
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let offers = source
        .offers
        .lock()
        .await
        .iter()
        .map(|(id, offer)| (*id, offer.clone()))
        .collect::<Vec<_>>();
    let mut visible = Vec::new();
    for (id, offer) in offers {
        let offer = &offer.offer;
        if source.current(offer) {
            let request: IndependentJoinRequest =
                serde_json::from_slice(&offer.payload).map_err(|_| StatusCode::CONFLICT)?;
            visible.push(serde_json::json!({"offer_id":id,"authenticated_endpoint":offer.remote_id.to_string(),"request":request}));
        }
    }
    Ok(Json(serde_json::json!(visible)))
}
#[utoipa::path(post, path = "/independent/node-joins/{id}/approve", operation_id = "approve", security(("authorization" = [])), params(("id" = String, Path)), responses((status = 202, description = "Operation-bound Node-join result"), (status = 403, description = "Invalid input or current authorization/operation denied"), (status = 409, description = "Invalid input or current authorization/operation denied"), (status = 503, description = "Invalid input or current authorization/operation denied")))]
async fn approve(
    State(source): State<Arc<IndependentJoinSource>>,
    Path(id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    source
        .actor(
            &headers,
            &[
                IdentityAction::DevicePairingUpdate,
                IdentityAction::ReplicaSignersEnroll,
            ],
        )
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let queued = source
        .offers
        .lock()
        .await
        .get(&id)
        .cloned()
        .ok_or(StatusCode::CONFLICT)?;
    if !source.current(&queued.offer) {
        return Err(StatusCode::CONFLICT);
    }
    if queued.accepted.swap(true, Ordering::SeqCst) {
        return Ok(StatusCode::ACCEPTED);
    }
    if source
        .approve
        .try_send(ApprovalWork { id, headers })
        .is_err()
    {
        queued.accepted.store(false, Ordering::SeqCst);
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    Ok(StatusCode::ACCEPTED)
}
async fn approve_work(source: &IndependentJoinSource, job: ApprovalWork) -> Result<(), StatusCode> {
    let ApprovalWork { id, headers } = job;
    let owner = source
        .actor(
            &headers,
            &[
                IdentityAction::DevicePairingUpdate,
                IdentityAction::ReplicaSignersEnroll,
            ],
        )
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let queued = source
        .offers
        .lock()
        .await
        .get(&id)
        .cloned()
        .ok_or(StatusCode::CONFLICT)?;
    let offer = &queued.offer;
    if !queued.accepted.load(Ordering::SeqCst) {
        return Err(StatusCode::CONFLICT);
    }
    if !source.current(offer) {
        return Err(StatusCode::CONFLICT);
    }
    let request: IndependentJoinRequest =
        serde_json::from_slice(&offer.payload).map_err(|_| StatusCode::BAD_REQUEST)?;
    source
        .check_request(&request)
        .map_err(|_| StatusCode::FORBIDDEN)?;
    if request.endpoint_id != offer.remote_id.to_string() {
        return Err(StatusCode::FORBIDDEN);
    }
    let approval = IndependentJoinApproval::sign(&source.identity, request, owner)
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let reply = serde_json::to_vec(&approval).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    if reply.len() > iroh_chain::MAX_PAIRING_PAYLOAD_LENGTH {
        return Err(StatusCode::BAD_REQUEST);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .as_secs() as i64;
    DbDeviceRepo::new(source.engine.clone())
        .enroll_approved_exact(
            approval.request.device_id,
            owner,
            &approval.request.operation.node,
            &approval.request.endpoint_id,
            now,
        )
        .await
        .map_err(|_| StatusCode::CONFLICT)?;
    let policies = DbSelectionPolicyRepo::new(source.engine.clone());
    if policies
        .get(approval.request.device_id, &owner.to_string(), Id::nil())
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .is_none()
    {
        policies
            .set_prevalidated(SelectionPolicy {
                device_id: approval.request.device_id,
                owner_subject: owner.to_string(),
                application_id: None,
                selected_kind: None,
                selected_id: None,
                admin_allowed: true,
            })
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    }
    source
        .verify(&approval, IdentityAction::ManagementSecuritySync)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let configuration = of_idp_client::apis::configuration::Configuration {
        base_path: source.idp.clone(),
        client: LocalTls::verification_client().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        bearer_access_token: Some(
            bearer(&headers)
                .map_err(|_| StatusCode::UNAUTHORIZED)?
                .to_owned(),
        ),
        ..Default::default()
    };
    of_idp_client::apis::independent_join_api::independent_join_enroll_node_join(
        &configuration,
        of_idp_client::apis::independent_join_api::IndependentJoinEnrollNodeJoinParams {
            independent_join_approval: serde_json::from_value(
                serde_json::to_value(&approval).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
            )
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        },
    )
    .await
    .map_err(|_| StatusCode::FORBIDDEN)?;
    if source
        .actor(
            &headers,
            &[
                IdentityAction::DevicePairingUpdate,
                IdentityAction::ReplicaSignersEnroll,
            ],
        )
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?
        != owner
        || !source.current(offer)
    {
        return Err(StatusCode::CONFLICT);
    }
    source
        .verify(&approval, IdentityAction::ManagementSecuritySync)
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;

    let _acceptance = source.acceptance_gate.lock().await;
    if !source.current(offer) {
        return Err(StatusCode::CONFLICT);
    }
    offer
        .reply(&reply)
        .await
        .map_err(|_| StatusCode::CONFLICT)?;
    {
        let mut answered = source.answered.lock().await;
        answered.retain(|_, offer| !offer.is_closed());
        answered.insert(id, offer.clone());
    }
    source.offers.lock().await.remove(&id);
    Ok(())
}
#[utoipa::path(post, path = "/independent/node-joins/{id}/reject", operation_id = "reject", security(("authorization" = [])), params(("id" = String, Path)), responses((status = 204, description = "Operation-bound Node-join result"), (status = 403, description = "Invalid input or current authorization/operation denied"), (status = 409, description = "Invalid input or current authorization/operation denied")))]
async fn reject(
    State(source): State<Arc<IndependentJoinSource>>,
    Path(id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    source
        .actor(
            &headers,
            &[
                IdentityAction::DevicePairingUpdate,
                IdentityAction::ReplicaSignersEnroll,
            ],
        )
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let queued = source
        .offers
        .lock()
        .await
        .get(&id)
        .cloned()
        .ok_or(StatusCode::CONFLICT)?;
    let offer = &queued.offer;
    let _acceptance = source.acceptance_gate.lock().await;
    if !source.current(offer) || queued.accepted.swap(true, Ordering::SeqCst) {
        return Err(StatusCode::CONFLICT);
    }
    offer.reply(b"").await.map_err(|_| StatusCode::CONFLICT)?;
    {
        let mut answered = source.answered.lock().await;
        answered.retain(|_, offer| !offer.is_closed());
        answered.insert(id, offer.clone());
    }
    source.offers.lock().await.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post, path = "/independent/node-joins/verify", operation_id = "verify", security(()), request_body = VerificationRequest, responses((status = 204, description = "Operation-bound Node-join result"), (status = 400, description = "Invalid input or current authorization/operation denied"), (status = 403, description = "Invalid input or current authorization/operation denied")))]
async fn verify(
    State(source): State<Arc<IndependentJoinSource>>,
    Json(body): Json<serde_json::Value>,
) -> StatusCode {
    let Ok(input) = serde_json::from_value::<VerificationRequest>(body) else {
        return StatusCode::BAD_REQUEST;
    };
    let action = match input.context.as_str() {
        "idp" => IdentityAction::IdpSecuritySync,
        "device" => IdentityAction::ManagementSecuritySync,
        _ => return StatusCode::BAD_REQUEST,
    };
    if source.verify(&input.approval, action).await.is_ok() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::FORBIDDEN
    }
}

fn join_routes() -> utoipa_axum::router::OpenApiRouter<Arc<IndependentJoinSource>> {
    utoipa_axum::router::OpenApiRouter::new()
        .routes(utoipa_axum::routes!(discover))
        .routes(utoipa_axum::routes!(accepting))
        .routes(utoipa_axum::routes!(set_accepting))
        .routes(utoipa_axum::routes!(pending))
        .routes(utoipa_axum::routes!(approve))
        .routes(utoipa_axum::routes!(reject))
        .routes(utoipa_axum::routes!(verify))
}

pub fn independent_join_openapi() -> utoipa::openapi::OpenApi {
    let mut document = join_routes().get_openapi().clone();
    utoipa::Modify::modify(&SecurityAddon, &mut document);
    document
}
