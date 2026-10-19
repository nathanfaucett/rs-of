use std::{
    collections::BTreeMap,
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use api::{IndependentPeer, LocalTls};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use db::Database;
use idp_model::{
    contract::{
        IdentityAction, IdentityResource, PermissionEvaluationRequest, PermissionSubject,
        PermissionTarget,
    },
    model::Id,
};
use iroh_chain::{EndpointIdStore, PairingOffer, Server};
use management_service::{
    DeviceIdentity, DeviceRepo, HostedControlPlane, LocalPermissions,
    independent_join::{IndependentJoinApproval, IndependentJoinRequest},
    join_history::{CheckJoinHistory, JoinHistoryContext, JoinHistorySource},
    replica::{DbDeviceRepo, DbSelectionPolicyRepo, SelectionPolicy},
};
use model::contract::PrincipalType;
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_util::sync::CancellationToken;

pub struct IndependentManagementRuntime {
    router: Router,
    protocol: iroh::protocol::Router,
    source: Arc<IndependentJoinSource>,
    task: JoinHandle<io::Result<()>>,
}
impl IndependentManagementRuntime {
    pub fn router(&self) -> Router {
        self.router.clone()
    }
    pub async fn shutdown(self) -> io::Result<()> {
        self.source.cancellation.cancel();
        self.task.await.map_err(io::Error::other)??;
        self.protocol.shutdown().await.map_err(io::Error::other)
    }
}

pub async fn build_independent_management(
    config: &crate::AppConfig,
    engine: Arc<Database>,
    plane: Arc<HostedControlPlane>,
    identity: Arc<DeviceIdentity>,
    issuer: &str,
    idp: &str,
    management: &str,
    storage: &str,
    cancellation: CancellationToken,
) -> io::Result<IndependentManagementRuntime> {
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
    let source = Arc::new(IndependentJoinSource {
        engine: engine.clone(),
        plane: plane.clone(),
        identity,
        issuer: issuer.into(),
        idp: idp.into(),
        management: management.into(),
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
        JoinHistoryContext::Management,
        std::path::Path::new(&config.data_dir).join("join-source-history"),
        check,
        source.cancellation.child_token(),
    );
    let routes = Router::new()
        .route("/node-joins/discover", post(discover))
        .route("/node-joins/acceptance", get(accepting).put(set_accepting))
        .route("/node-joins/pending", get(pending))
        .route("/node-joins/{id}/approve", post(approve))
        .route("/node-joins/{id}/reject", post(reject))
        .route("/node-joins/verify", post(verify))
        .with_state(source.clone())
        .merge(history.router())
        .layer(cors);
    let routes = if config.server.prefix().is_empty() {
        routes
    } else {
        Router::new().nest(config.server.prefix(), routes)
    };
    let router = crate::build_router(engine, management, config.server.prefix(), storage, plane)
        .await
        .map_err(io::Error::other)?
        .merge(routes);
    Ok(IndependentManagementRuntime {
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
    engine: Arc<Database>,
    plane: Arc<HostedControlPlane>,
    identity: Arc<DeviceIdentity>,
    issuer: String,
    idp: String,
    management: String,
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
            || request.source.management != self.management
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
            permission(&self.engine, owner, *action).await?;
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
        permission(&self.engine, approval.owner_subject, action).await
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

async fn discover(
    State(source): State<Arc<IndependentJoinSource>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<IndependentPeer>, StatusCode> {
    if !source.server.pairing_enabled() {
        return Err(StatusCode::FORBIDDEN);
    }
    let nonce = body["nonce"]
        .as_str()
        .filter(|nonce| nonce.len() == 64 && nonce.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or(StatusCode::BAD_REQUEST)?;
    source
        .plane
        .get_storage_endpoint_identity(&source.identity.endpoint_id().to_string())
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let mut peer = IndependentPeer {
        nonce: nonce.into(),
        issuer: source.issuer.clone(),
        idp: source.idp.clone(),
        management: source.management.clone(),
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
    let enabled = body["accepting"].as_bool().ok_or(StatusCode::BAD_REQUEST)?;
    let _acceptance = source.acceptance_gate.lock().await;
    source.server.set_pairing_enabled(enabled);
    Ok(StatusCode::NO_CONTENT)
}
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

    let response = LocalTls::verification_client()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .post(format!("{}/node-joins/enroll", source.idp))
        .bearer_auth(bearer(&headers).map_err(|_| StatusCode::UNAUTHORIZED)?)
        .json(&approval)
        .send()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    if !response.status().is_success() {
        return Err(StatusCode::FORBIDDEN);
    }
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
async fn verify(
    State(source): State<Arc<IndependentJoinSource>>,
    Json(body): Json<serde_json::Value>,
) -> StatusCode {
    let Ok(approval) = serde_json::from_value::<IndependentJoinApproval>(body["approval"].clone())
    else {
        return StatusCode::BAD_REQUEST;
    };
    let action = match body["context"].as_str() {
        Some("idp") => IdentityAction::IdpSecuritySync,
        Some("management") => IdentityAction::ManagementSecuritySync,
        _ => return StatusCode::BAD_REQUEST,
    };
    if source.verify(&approval, action).await.is_ok() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::FORBIDDEN
    }
}
