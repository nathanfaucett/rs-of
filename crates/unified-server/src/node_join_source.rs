use std::{collections::BTreeMap, io, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use db::Database;
use idp_model::model::Id;
use idp_server::{IdpRuntime, NodeJoinApproval, NodeJoinRequest, OwnerProvisioner};
use iroh_chain::PairingOffer;
use management_service::{DeviceRepo, replica::DbDeviceRepo};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{Mutex, broadcast},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use crate::{SecurityProtocol, UnifiedEndpoint};

struct QueuedOffer {
    offer: PairingOffer,
    replied: CancellationToken,
    operation: Mutex<()>,
    #[cfg(test)]
    invalidated: tokio::sync::Notify,
}

impl QueuedOffer {
    async fn released(&self, mut acceptance: tokio::sync::watch::Receiver<Option<u64>>) {
        tokio::select! {
            () = self.offer.closed() => (),
            () = self.replied.cancelled() => self.offer.closed().await,
            () = async {
                let _ = acceptance.wait_for(|current| *current != Some(self.offer.acceptance_generation)).await;
            } => {
                #[cfg(test)]
                self.invalidated.notify_one();
                let replied = {
                    let _operation = self.operation.lock().await;
                    self.replied.is_cancelled()
                };
                if replied {
                    self.offer.closed().await;
                }
            },
        }
    }
}

struct Pending {
    offers: BTreeMap<Id, Arc<QueuedOffer>>,
    answered: BTreeMap<Id, PairingOffer>,
}

impl Pending {
    fn finish_reply(&mut self, id: Id, queued: &Arc<QueuedOffer>, cancelled: bool) {
        queued.replied.cancel();
        if self
            .offers
            .get(&id)
            .is_some_and(|current| Arc::ptr_eq(current, queued))
        {
            self.offers.remove(&id);
        }
        if !queued.offer.is_closed() && !cancelled {
            self.answered.insert(id, queued.offer.clone());
        }
    }
}

pub(crate) struct NodeJoinSource {
    endpoint: Arc<UnifiedEndpoint>,
    idp: Arc<IdpRuntime>,
    owner: Arc<OwnerProvisioner>,
    security: SecurityProtocol,
    management: Arc<Database>,
    installation: String,
    cancellation: CancellationToken,
    receiver: Mutex<broadcast::Receiver<PairingOffer>>,
    pending: Mutex<Pending>,
}

impl NodeJoinSource {
    pub(crate) fn new(
        endpoint: Arc<UnifiedEndpoint>,
        idp: Arc<IdpRuntime>,
        owner: Arc<OwnerProvisioner>,
        security: SecurityProtocol,
        management: Arc<Database>,
        installation: String,
        cancellation: CancellationToken,
    ) -> Self {
        let receiver = endpoint.server().subscribe_pairing_offers();
        Self {
            endpoint,
            idp,
            owner,
            security,
            management,
            installation,
            cancellation,
            receiver: Mutex::new(receiver),
            pending: Mutex::new(Pending {
                offers: BTreeMap::new(),
                answered: BTreeMap::new(),
            }),
        }
    }

    pub(crate) fn router(self: &Arc<Self>) -> Router {
        Router::new()
            .route(
                "/management/node-joins/acceptance",
                get(acceptance).put(set_acceptance),
            )
            .route("/management/node-joins/pending", get(pending))
            .route("/management/node-joins/{id}/approve", post(approve))
            .route("/management/node-joins/{id}/reject", post(reject))
            .with_state(self.clone())
    }

    pub(crate) async fn run(&self) -> io::Result<()> {
        let mut receiver = self.receiver.lock().await;
        let mut acceptance = self.endpoint.server().subscribe_pairing_acceptance();
        let mut lifetimes = JoinSet::new();
        loop {
            let offer = tokio::select! {
                () = self.cancellation.cancelled() => break,
                changed = acceptance.changed() => {
                    if changed.is_err() { break; }
                    self.prune().await;
                    continue;
                },
                released = lifetimes.join_next(), if !lifetimes.is_empty() => {
                    let id = released.expect("nonempty offer lifetime tasks").map_err(io::Error::other)?;
                    let mut pending = self.pending.lock().await;
                    pending.offers.remove(&id);
                    pending.answered.remove(&id);
                    continue;
                },
                event = receiver.recv() => match event {
                    Ok(offer) => offer,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                },
            };
            let result = tokio::select! {
                () = self.cancellation.cancelled() => break,
                result = self.process_offer(offer) => result,
            };
            match result {
                Ok(Some((id, queued))) => {
                    let acceptance = self.endpoint.server().subscribe_pairing_acceptance();
                    lifetimes.spawn(async move {
                        queued.released(acceptance).await;
                        id
                    });
                }
                Ok(None) => (),
                Err(_) => log::warn!(
                    "purpose-bound Node offer processing failed; no enrollment was performed"
                ),
            }
        }
        lifetimes.abort_all();
        while lifetimes.join_next().await.is_some() {}
        self.clear().await;
        Ok(())
    }

    fn display(&self, offer: &PairingOffer) -> io::Result<NodeJoinRequest> {
        let request: NodeJoinRequest = serde_json::from_slice(&offer.payload)?;
        request.validate()?;
        if request.installation != self.installation
            || request.approving_peer != self.endpoint.endpoint_id().to_string()
            || request.endpoint_id != offer.remote_id.to_string()
        {
            return Err(denied(
                "join Installation or authenticated endpoint mismatch",
            ));
        }
        Ok(request)
    }

    async fn ready(&self) -> io::Result<bool> {
        self.security
            .local_node_public_ready(self.owner.as_ref())
            .await
    }

    fn active(&self) -> bool {
        self.endpoint
            .server()
            .pairing_acceptance_generation()
            .is_some()
            && !self.cancellation.is_cancelled()
    }

    fn active_offer(&self, offer: &PairingOffer) -> bool {
        self.active()
            && self.endpoint.server().pairing_acceptance_generation()
                == Some(offer.acceptance_generation)
            && !offer.is_closed()
    }

    async fn prune(&self) {
        let mut pending = self.pending.lock().await;
        pending
            .offers
            .retain(|_, queued| self.active_offer(&queued.offer));
        pending.answered.retain(|_, offer| !offer.is_closed());
    }

    async fn process_offer(
        &self,
        offer: PairingOffer,
    ) -> io::Result<Option<(Id, Arc<QueuedOffer>)>> {
        if !self.active_offer(&offer) {
            return Ok(None);
        }
        let discovery = crate::issuer_discovery::is_issuer_discovery(&offer);
        if !discovery && self.display(&offer).is_err() {
            return Ok(None);
        }
        if !self.ready().await? || !self.active_offer(&offer) {
            return Ok(None);
        }
        let id = Id::now_v7();
        let queued = Arc::new(QueuedOffer {
            offer: offer.clone(),
            replied: CancellationToken::new(),
            operation: Mutex::new(()),
            #[cfg(test)]
            invalidated: tokio::sync::Notify::new(),
        });
        if discovery {
            crate::issuer_discovery::answer_issuer_discovery(
                &offer,
                &self.endpoint.endpoint_id().to_string(),
                &self.installation,
            )
            .await?;
            queued.replied.cancel();
            if !offer.is_closed() {
                self.pending.lock().await.answered.insert(id, offer.clone());
            }
        } else {
            let mut pending = self.pending.lock().await;
            if self.active_offer(&offer) {
                pending.offers.insert(id, queued.clone());
            }
        }
        Ok(Some((id, queued)))
    }

    pub(crate) async fn accepting(
        &self,
        bearer: &str,
        accepting: Option<bool>,
    ) -> io::Result<bool> {
        if self.cancellation.is_cancelled() {
            return Err(denied("runtime is shutting down"));
        }
        let accepting = self.idp.node_join_accepting(bearer, accepting).await?;
        self.prune().await;
        Ok(accepting)
    }

    pub(crate) async fn pending(&self, bearer: &str) -> io::Result<serde_json::Value> {
        self.idp.authorize_node_join_approver(bearer).await?;
        self.prune().await;
        let pending = self.pending.lock().await;
        let mut display = Vec::new();
        for (id, offer) in &pending.offers {
            display.push(serde_json::json!({"offer_id": id, "authenticated_endpoint": offer.offer.remote_id.to_string(), "request": self.display(&offer.offer)?}));
        }
        Ok(serde_json::json!(display))
    }

    pub(crate) async fn approve(&self, bearer: &str, id: Id) -> io::Result<NodeJoinApproval> {
        #[cfg(test)]
        let started = std::time::Instant::now();
        let queued = match self.queued_offer(id).await {
            Ok(queued) => queued,
            Err(error) => {
                self.idp.authorize_node_join_approver(bearer).await?;
                return Err(error);
            }
        };
        let _operation = queued.operation.lock().await;
        self.require_current_offer(id, &queued).await?;
        let offer = &queued.offer;
        self.display(offer)?;
        #[cfg(test)]
        eprintln!(
            "node join approval stage=enrollment_begin elapsed={:?}",
            started.elapsed()
        );
        let approval = self.idp.enroll_node_join_offer(offer, bearer).await?;
        #[cfg(test)]
        eprintln!(
            "node join approval stage=enrollment_verified elapsed={:?}",
            started.elapsed()
        );
        self.require_current_offer(id, &queued).await?;
        self.idp.verify_node_join_enrollment(&approval).await?;
        let identity = DbDeviceRepo::new(self.management.clone())
            .find_approved_by_public_key(&offer.remote_id.to_string())
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| denied("enrolled endpoint has no approved Management Device"))?;
        if identity.id != approval.request.member_id
            || identity.owner_subject != approval.owner_subject.to_string()
        {
            return Err(denied(
                "enrolled Device owner/binding differs from approval",
            ));
        }
        let policies =
            management_service::replica::DbSelectionPolicyRepo::new(self.management.clone());
        if policies
            .get(identity.id, &identity.owner_subject, Id::nil())
            .await
            .map_err(io::Error::other)?
            .is_none()
        {
            policies
                .set_prevalidated(management_service::replica::SelectionPolicy {
                    device_id: identity.id,
                    owner_subject: identity.owner_subject,
                    application_id: None,
                    selected_kind: None,
                    selected_id: None,
                    admin_allowed: true,
                })
                .await
                .map_err(io::Error::other)?;
        }
        self.endpoint.refresh_approved_peers(&self.idp).await?;
        self.require_current_offer(id, &queued).await?;
        self.idp.reply_node_join(offer, bearer, &approval).await?;
        self.finish_offer(id, &queued).await;
        #[cfg(test)]
        eprintln!(
            "node join approval stage=reply_published elapsed={:?}",
            started.elapsed()
        );
        Ok(approval)
    }

    pub(crate) async fn reject(&self, bearer: &str, id: Id) -> io::Result<()> {
        let queued = match self.queued_offer(id).await {
            Ok(queued) => queued,
            Err(error) => {
                self.idp.authorize_node_join_approver(bearer).await?;
                return Err(error);
            }
        };
        let _operation = queued.operation.lock().await;
        self.idp.authorize_node_join_approver(bearer).await?;
        self.require_current_offer(id, &queued).await?;
        let result = queued.offer.reply(b"").await;
        self.finish_offer(id, &queued).await;
        result
    }

    async fn queued_offer(&self, id: Id) -> io::Result<Arc<QueuedOffer>> {
        self.prune().await;
        if !self.active() {
            return Err(denied("source acceptance has ended"));
        }
        self.pending
            .lock()
            .await
            .offers
            .get(&id)
            .cloned()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "pending authenticated offer unavailable",
                )
            })
    }

    async fn require_current_offer(&self, id: Id, queued: &Arc<QueuedOffer>) -> io::Result<()> {
        let pending = self.pending.lock().await;
        if !self.active_offer(&queued.offer)
            || !pending
                .offers
                .get(&id)
                .is_some_and(|current| Arc::ptr_eq(current, queued))
        {
            return Err(denied("source acceptance/pending offer has ended"));
        }
        Ok(())
    }

    async fn finish_offer(&self, id: Id, queued: &Arc<QueuedOffer>) {
        self.pending
            .lock()
            .await
            .finish_reply(id, queued, self.cancellation.is_cancelled());
    }

    #[cfg(test)]
    pub(crate) async fn retained_offer_count(&self) -> usize {
        let pending = self.pending.lock().await;
        pending.offers.len() + pending.answered.len()
    }

    pub(crate) async fn clear(&self) {
        let mut pending = self.pending.lock().await;
        pending.offers.clear();
        pending.answered.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{Pending, QueuedOffer};
    use idp_model::model::Id;
    use iroh::{
        endpoint::Connection,
        protocol::{AcceptError, ProtocolHandler},
    };
    use iroh_chain::{EndpointIdStore, PAIRING_ALPN, Server};
    use std::{collections::BTreeMap, sync::Arc, time::Duration};
    use tokio::sync::{Mutex, Notify};
    use tokio_util::sync::CancellationToken;

    #[derive(Debug)]
    struct UnusedDataProtocol;
    impl ProtocolHandler for UnusedDataProtocol {
        async fn accept(&self, _: Connection) -> Result<(), AcceptError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn acceptance_invalidation_during_reply_retains_until_transport_closes() {
        tokio::time::timeout(Duration::from_secs(15), async {
            let source = Server::bind(crate::endpoint::TestPreset, EndpointIdStore::default())
                .await
                .expect("isolated source transport");
            let peer = Server::bind(crate::endpoint::TestPreset, EndpointIdStore::default())
                .await
                .expect("isolated peer transport");
            let router = source.router(UnusedDataProtocol);
            peer.peers().add(source.endpoint().id());
            source.set_pairing_enabled(true);
            let mut offers = source.subscribe_pairing_offers();
            let connection = peer
                .endpoint()
                .connect(source.endpoint().addr(), PAIRING_ALPN)
                .await
                .expect("authenticated pairing connection");
            let (mut send, mut recv) = connection.open_bi().await.expect("pairing stream");
            send.write_all(b"lifecycle offer")
                .await
                .expect("offer bytes");
            send.finish().expect("finish offer");
            let queued = Arc::new(QueuedOffer {
                offer: offers.recv().await.expect("actual transport offer"),
                replied: CancellationToken::new(),
                operation: Mutex::new(()),
                invalidated: Notify::new(),
            });
            let id = Id::now_v7();
            let pending = Arc::new(Mutex::new(Pending {
                offers: BTreeMap::from([(id, queued.clone())]),
                answered: BTreeMap::new(),
            }));
            let lifetime = tokio::spawn({
                let queued = queued.clone();
                let pending = pending.clone();
                let acceptance = source.subscribe_pairing_acceptance();
                async move {
                    queued.released(acceptance).await;
                    let mut pending = pending.lock().await;
                    pending.offers.remove(&id);
                    pending.answered.remove(&id);
                }
            });
            let published = Arc::new(Notify::new());
            let resume = Arc::new(Notify::new());
            let replying = tokio::spawn({
                let queued = queued.clone();
                let pending = pending.clone();
                let published = published.clone();
                let resume = resume.clone();
                async move {
                    let _operation = queued.operation.lock().await;
                    queued
                        .offer
                        .reply(b"complete reply")
                        .await
                        .expect("publish reply");
                    published.notify_one();
                    resume.notified().await;
                    pending.lock().await.finish_reply(id, &queued, false);
                }
            });
            published.notified().await;
            assert!(
                !queued.replied.is_cancelled(),
                "pause before reply-retention transition"
            );
            source.set_pairing_enabled(false);
            queued.invalidated.notified().await;
            pending.lock().await.offers.clear();
            assert!(
                !lifetime.is_finished(),
                "invalidation must not reap an in-flight reply's closure owner"
            );
            resume.notify_one();
            replying.await.expect("finish reply transition");
            assert_eq!(
                recv.read_to_end(64).await.expect("untruncated reply"),
                b"complete reply"
            );
            assert_eq!(pending.lock().await.answered.len(), 1);
            assert!(
                !lifetime.is_finished(),
                "published reply retains a live closure watcher"
            );
            connection.close(0u32.into(), b"reply consumed");
            lifetime.await.expect("closure watcher and reaper complete");
            let retained = pending.lock().await;
            assert!(retained.offers.is_empty() && retained.answered.is_empty());
            drop(retained);
            router
                .shutdown()
                .await
                .expect("close source protocol router");
            source.close().await;
            peer.close().await;
        })
        .await
        .expect("bounded deterministic reply lifecycle regression");
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Acceptance {
    accepting: bool,
}

type HttpError = (StatusCode, Json<serde_json::Value>);
fn bearer(headers: &HeaderMap) -> Result<&str, HttpError> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "bearer_required"})),
            )
        })
}
fn http_error(error: io::Error) -> HttpError {
    let status = match error.kind() {
        io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
        io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => StatusCode::CONFLICT,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    (
        status,
        Json(serde_json::json!({"error": "node_join_request_denied"})),
    )
}
async fn acceptance(
    State(source): State<Arc<NodeJoinSource>>,
    headers: HeaderMap,
) -> Result<Json<Acceptance>, HttpError> {
    Ok(Json(Acceptance {
        accepting: source
            .accepting(bearer(&headers)?, None)
            .await
            .map_err(http_error)?,
    }))
}
async fn set_acceptance(
    State(source): State<Arc<NodeJoinSource>>,
    headers: HeaderMap,
    Json(input): Json<Acceptance>,
) -> Result<Json<Acceptance>, HttpError> {
    Ok(Json(Acceptance {
        accepting: source
            .accepting(bearer(&headers)?, Some(input.accepting))
            .await
            .map_err(http_error)?,
    }))
}
async fn pending(
    State(source): State<Arc<NodeJoinSource>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, HttpError> {
    Ok(Json(
        source
            .pending(bearer(&headers)?)
            .await
            .map_err(http_error)?,
    ))
}
async fn approve(
    State(source): State<Arc<NodeJoinSource>>,
    Path(id): Path<Id>,
    headers: HeaderMap,
) -> Result<Json<NodeJoinApproval>, HttpError> {
    Ok(Json(
        source
            .approve(bearer(&headers)?, id)
            .await
            .map_err(http_error)?,
    ))
}
async fn reject(
    State(source): State<Arc<NodeJoinSource>>,
    Path(id): Path<Id>,
    headers: HeaderMap,
) -> Result<StatusCode, HttpError> {
    source
        .reject(bearer(&headers)?, id)
        .await
        .map_err(http_error)?;
    Ok(StatusCode::NO_CONTENT)
}
fn denied(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}
