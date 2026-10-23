use std::{future::Future, io, pin::Pin, sync::Arc};

use db::DatabaseTransport;
use idp_model::{
    contract::{IdpSignerRecord, ReplicaMembership},
    model::Id,
};
use idp_service::replica::LocalIdentity;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use ofdb_sql::{
    SessionConfig, SyncDirection, SyncError, SyncResult, SyncRole, SyncTransport,
    SyncValidationDirection,
};
use serde::{Deserialize, Serialize};

use crate::DeviceRepo;
use crate::hosted_join::NodeJoinApproval;
use crate::node_join_owner::{NodeJoinOwner, NodeJoinStage};
use crate::replica::DbDeviceRepo;
use crate::security_candidates::{JoinCandidate, PeerBinding};

use super::{SecurityContext, SecurityProtocol, denied, now};

pub const BOOTSTRAP_ALPN: &[u8] = b"idp-bootstrap/1";

const BOOTSTRAP_STREAM_KIND: u8 = 4;
const PURPOSE: &str = "of.equal-node-public-bootstrap.v1";

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct BootstrapRequest {
    purpose: String,
    context: SecurityContext,
    approval: NodeJoinApproval,
}

impl SecurityProtocol {
    /// Drives only purpose-bound public bootstrap; the host must verify all local services separately.
    pub async fn join_from_trusted_peer(
        &self,
        owner: &impl NodeJoinOwner,
        name: &str,
        peer: &str,
    ) -> io::Result<NodeJoinApproval> {
        let peer: iroh::EndpointId = peer.parse().map_err(io::Error::other)?;
        let binding = self
            .binding
            .as_ref()
            .ok_or_else(|| denied("local independent signer binding is required"))?;
        let request = owner.prepare_node_join(binding, name, peer)?;
        if owner.node_join_approval()?.is_none() {
            let approval = request.send(&self.server, peer.into()).await?;
            owner.record_node_join_approval(&approval)?;
        }
        let approval = owner.node_join_anchor()?;
        bootstrap_stage(|| self.check_join_local_binding(owner)).await?;
        if owner.node_join_stage_recorded(NodeJoinStage::Idp)? {
            self.check_join_identity(&approval).await?;
        }
        if owner.node_join_stage_recorded(NodeJoinStage::Management)? {
            self.check_exact_join_trust(&approval).await?;
            self.check_join_context_grants(&approval).await?;
            self.refresh_learned_membership().await?;
        }
        if self.node_join_public_ready(owner).await? {
            return Ok(approval);
        }
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            bootstrap_stage(|| self.check_join_local_binding(owner)).await?;
            if owner.node_join_anchor()? != approval {
                return Err(denied("join anchor changed during admission refresh"));
            }
            let attempt = async {
                let connection = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    self.server.endpoint().connect(peer, BOOTSTRAP_ALPN),
                )
                .await
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::TimedOut,
                        "trusted peer bootstrap handshake timed out",
                    )
                })?
                .map_err(io::Error::other)?;
                let result = async {
                    for context in [SecurityContext::Idp, SecurityContext::Device] {
                        if !owner.node_join_stage_recorded(stage(context))? {
                            bootstrap_stage(|| {
                                self.receive_node_bootstrap(&connection, owner, context)
                            })
                            .await
                            .inspect_err(|_error| {
                                #[cfg(test)]
                                eprintln!("bootstrap receiver stage=receive context={context:?} error={_error}");
                            })?;
                        }
                    }
                    bootstrap_stage(|| self.complete_node_join_public_bootstrap(owner))
                        .await
                        .inspect_err(|_error| {
                            #[cfg(test)]
                            eprintln!("bootstrap receiver stage=complete_public error={_error}");
                        })
                }
                .await;
                connection.close(0u32.into(), b"public bootstrap finished");
                result
            };
            match attempt.await {
                Ok(()) => return Ok(approval),
                Err(error) => {
                    #[cfg(test)]
                    eprintln!("bootstrap receiver stage=attempt error={error}");
                    if tokio::time::Instant::now() >= deadline {
                        return Err(error);
                    }
                    if owner.node_join_stage_recorded(NodeJoinStage::Idp)? {
                        self.check_join_identity(&approval).await?;
                    }
                    if owner.node_join_stage_recorded(NodeJoinStage::Management)? {
                        self.check_exact_join_trust(&approval).await?;
                        self.check_join_context_grants(&approval).await?;
                        self.refresh_learned_membership().await?;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    }

    async fn check_join_local_binding(&self, owner: &impl NodeJoinOwner) -> io::Result<()> {
        owner.check_node_join_local_binding().await?;
        let approval = owner.node_join_anchor()?;
        DbDeviceRepo::new(self.management.clone().client())
            .check_pending_approved_binding(
                approval.request.member_id,
                approval.owner_subject,
                &approval.request.name,
                &approval.request.endpoint_id,
            )
            .await
            .map_err(denied)
    }

    async fn refresh_learned_membership(&self) -> io::Result<()> {
        let devices = DbDeviceRepo::new(self.management.clone().client());
        let identity = LocalIdentity::new(self.idp.client());
        let mut peers = Vec::new();
        for device in devices.list().await.map_err(denied)? {
            if devices
                .find_approved_by_public_key(&device.public_key)
                .await
                .map_err(denied)?
                .is_none()
            {
                continue;
            }
            let owner: Id = device.owner_subject.parse().map_err(denied)?;
            if !identity.is_active_user(owner).await? {
                continue;
            }
            peers.push(
                device
                    .public_key
                    .parse::<iroh::EndpointId>()
                    .map_err(denied)?,
            );
        }
        self.server.peers().replace(peers);
        Ok(())
    }

    pub fn node_bootstrap_handler(&self) -> impl iroh::protocol::ProtocolHandler + use<> {
        NodeBootstrapHandler {
            protocol: self.clone(),
        }
    }

    pub async fn receive_node_bootstrap(
        &self,
        connection: &Connection,
        owner: &impl NodeJoinOwner,
        context: SecurityContext,
    ) -> io::Result<SyncResult> {
        #[cfg(test)]
        let started = std::time::Instant::now();
        let approval = owner.node_join_anchor()?;
        bootstrap_stage(|| self.check_bootstrap_receiver(connection, owner, &approval)).await?;
        if context == SecurityContext::Device
            && !owner.node_join_stage_recorded(NodeJoinStage::Idp)?
        {
            return Err(denied("Management bootstrap requires the IdP import first"));
        }
        self.validate_local_schema(context).await?;
        #[cfg(test)]
        eprintln!(
            "bootstrap receiver stage=schema context={context:?} elapsed_ms={}",
            started.elapsed().as_millis()
        );
        let request = BootstrapRequest {
            purpose: PURPOSE.into(),
            context,
            approval: approval.clone(),
        };
        let (mut send, recv) = connection.open_bi().await.map_err(io::Error::other)?;
        bootstrap_stage(|| self.check_bootstrap_receiver(connection, owner, &approval)).await?;
        send.write_all(&[BOOTSTRAP_STREAM_KIND])
            .await
            .map_err(io::Error::other)?;
        let mut transport = DatabaseTransport::new(send, recv);
        bootstrap_stage(|| self.check_bootstrap_receiver(connection, owner, &approval)).await?;
        transport.send(serde_json::to_vec(&request)?).await?;
        let reply = tokio::time::timeout(std::time::Duration::from_secs(10), transport.receive())
            .await
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "trusted peer bootstrap admission acknowledgment timed out",
                )
            })??;
        let accepted: BootstrapRequest = serde_json::from_slice(&reply)?;
        if accepted != request {
            return Err(denied("bootstrap context/anchor reply mismatch"));
        }
        let result = bootstrap_stage(|| {
            self.receive_bootstrap_history(
                connection,
                owner,
                &approval,
                context,
                &mut transport,
                || async { Ok(()) },
            )
        })
        .await?;
        bootstrap_stage(|| self.check_bootstrap_receiver(connection, owner, &approval)).await?;
        #[cfg(test)]
        eprintln!(
            "bootstrap receiver stage=imported context={context:?} elapsed_ms={}",
            started.elapsed().as_millis()
        );
        owner.record_node_join_stage(stage(context))?;
        self.check_join_identity(&approval).await?;
        if context == SecurityContext::Device {
            self.check_exact_join_trust(&approval).await?;
            self.check_join_context_grants(&approval).await?;
            self.refresh_learned_membership().await?;
        }
        Ok(result)
    }

    pub(super) async fn accept_node_bootstrap(
        &self,
        connection: Connection,
        send: SendStream,
        recv: RecvStream,
    ) -> io::Result<()> {
        let mut transport = DatabaseTransport::new(send, recv);
        let request: BootstrapRequest = serde_json::from_slice(&transport.receive().await?)?;
        if request.purpose != PURPOSE {
            return Err(denied("invalid bootstrap purpose"));
        }
        #[cfg(test)]
        let started = std::time::Instant::now();
        self.authorize_bootstrap_source(&connection, &request.approval, request.context)
            .await?;
        self.validate_local_schema(request.context).await?;
        #[cfg(test)]
        eprintln!(
            "bootstrap source stage=schema context={:?} elapsed_ms={}",
            request.context,
            started.elapsed().as_millis()
        );
        self.authorize_bootstrap_source(&connection, &request.approval, request.context)
            .await?;
        transport.send(serde_json::to_vec(&request)?).await?;
        let mut transport = SourceTransport {
            inner: &mut transport,
            protocol: self,
            connection: &connection,
            approval: &request.approval,
            context: request.context,
            #[cfg(test)]
            last_send: std::time::Instant::now(),
        };
        bootstrap_stage(|| {
            self.node_bootstrap_session(
                &request.approval,
                request.context,
                SyncDirection::SendOnly,
                &mut transport,
                || async {
                    self.authorize_bootstrap_source(&connection, &request.approval, request.context)
                        .await
                        .map_err(|_| "bootstrap source no longer authorized".into())
                },
            )
        })
        .await
        .inspect_err(|_error| {
            #[cfg(test)]
            eprintln!(
                "bootstrap source stage=sync context={:?} error={_error}",
                request.context
            );
        })?;
        Ok(())
    }

    async fn authorize_bootstrap_source(
        &self,
        connection: &Connection,
        approval: &NodeJoinApproval,
        context: SecurityContext,
    ) -> io::Result<()> {
        if connection.alpn() != BOOTSTRAP_ALPN {
            return Err(denied("bootstrap requires its purpose-specific ALPN"));
        }
        approval.verify(&approval.request).map_err(denied)?;
        let request = &approval.request;
        if request.installation != self.installation
            || request.approving_peer != self.server.endpoint().id().to_string()
            || request.endpoint_id != connection.remote_id().to_string()
        {
            return Err(denied(
                "bootstrap Installation or authenticated endpoint mismatch",
            ));
        }
        self.check_exact_join_trust(approval).await?;
        let local = self
            .binding
            .as_ref()
            .ok_or_else(|| denied("local node signer is not configured"))?;
        let endpoint = self.server.endpoint().id().to_string();
        if local.endpoint_id != endpoint {
            return Err(denied("local node signer/member is not currently trusted"));
        }
        LocalIdentity::new(self.idp.client())
            .check_signer_binding(&self.installation, local, now()?)
            .await?;
        self.owner()
            .check_permission(approval.owner_subject, context)
            .await?;
        self.owner()
            .binding(&endpoint, Some(local.member_id))
            .await?;
        if self.cancellation.is_cancelled() {
            return Err(denied("bootstrap is shutting down"));
        }
        Ok(())
    }

    async fn check_join_identity(
        &self,
        approval: &NodeJoinApproval,
    ) -> io::Result<Vec<(ReplicaMembership, IdpSignerRecord)>> {
        let request = &approval.request;
        LocalIdentity::new(self.idp.client())
            .check_join_identity(
                &self.installation,
                request.member_id,
                &request.endpoint_id,
                &request.public_signer,
                approval.owner_subject,
                now()?,
            )
            .await
    }

    pub async fn check_exact_join_trust(
        &self,
        approval: &NodeJoinApproval,
    ) -> io::Result<Vec<(ReplicaMembership, IdpSignerRecord)>> {
        let trusted = self.check_join_identity(approval).await?;
        let request = &approval.request;
        let device = DbDeviceRepo::new(self.management.clone().client())
            .find_approved_by_public_key(&request.endpoint_id)
            .await
            .map_err(denied)?
            .ok_or_else(|| denied("bootstrap requires exact approved Device"))?;
        if device.id != request.member_id
            || approval.owner_subject.is_nil()
            || device.owner_subject != approval.owner_subject.to_string()
            || device.name != request.name
        {
            return Err(denied(
                "bootstrap Device owner/name/endpoint binding changed",
            ));
        }
        Ok(trusted)
    }

    pub async fn check_bootstrap_receiver(
        &self,
        connection: &Connection,
        owner: &impl NodeJoinOwner,
        approval: &NodeJoinApproval,
    ) -> io::Result<()> {
        if connection.alpn() != BOOTSTRAP_ALPN
            || self.cancellation.is_cancelled()
            || !Arc::ptr_eq(&self.idp, &owner.node_join_database()?)
            || owner.node_join_anchor()? != *approval
            || approval.request.installation != self.installation
            || approval.request.endpoint_id != self.server.endpoint().id().to_string()
            || approval.request.approving_peer != connection.remote_id().to_string()
        {
            return Err(denied(
                "bootstrap receiver requires persisted exact local anchor and pinned connection",
            ));
        }
        bootstrap_stage(|| self.check_join_local_binding(owner))
            .await
            .map_err(denied)?;
        if owner.node_join_stage_recorded(NodeJoinStage::Idp)? {
            self.check_join_identity(approval).await?;
        }
        if owner.node_join_stage_recorded(NodeJoinStage::Management)? {
            self.check_exact_join_trust(approval).await?;
            self.check_join_context_grants(approval).await?;
        }
        if self.cancellation.is_cancelled() {
            return Err(denied("bootstrap is shutting down"));
        }
        Ok(())
    }

    /// Receives only public bootstrap history, checking current admission per send and apply.
    /// The additional apply check can deny an admitted batch, never grant admission.
    pub async fn receive_bootstrap_history<T, Q, U>(
        &self,
        connection: &Connection,
        owner: &impl NodeJoinOwner,
        approval: &NodeJoinApproval,
        context: SecurityContext,
        transport: &mut T,
        mut check: Q,
    ) -> io::Result<SyncResult>
    where
        T: SyncTransport<Error = io::Error>,
        Q: FnMut() -> U + Send,
        U: Future<Output = Result<(), String>> + Send,
    {
        self.check_bootstrap_receiver(connection, owner, approval)
            .await?;
        if context == SecurityContext::Device
            && !owner.node_join_stage_recorded(NodeJoinStage::Idp)?
        {
            return Err(denied("Management bootstrap requires the IdP import first"));
        }
        self.validate_local_schema(context).await?;
        let mut transport = ReceiverTransport {
            inner: transport,
            protocol: self,
            connection,
            owner,
            approval,
        };
        self.node_bootstrap_session(
            approval,
            context,
            SyncDirection::ReceiveOnly,
            &mut transport,
            || {
                let additional = check();
                async move {
                    bootstrap_stage(|| self.check_bootstrap_receiver(connection, owner, approval))
                        .await
                        .map_err(|_| "bootstrap receiver no longer authorized".to_owned())?;
                    additional.await
                }
            },
        )
        .await
    }

    async fn node_bootstrap_session<T, Q, U>(
        &self,
        approval: &NodeJoinApproval,
        context: SecurityContext,
        direction: SyncDirection,
        transport: &mut T,
        mut check: Q,
    ) -> io::Result<SyncResult>
    where
        T: SyncTransport<Error = io::Error>,
        Q: FnMut() -> U + Send,
        U: Future<Output = Result<(), String>> + Send,
    {
        let candidate = JoinCandidate {
            member_id: approval.request.member_id,
            device_id: approval.request.member_id,
            endpoint: &approval.request.endpoint_id,
            owner: approval.owner_subject,
            public_signer: &approval.request.public_signer,
            name: &approval.request.name,
        };
        let mut total = SyncResult::default();
        for (database, history, tables) in self.sessions(context) {
            let directory = self
                .staging_directory
                .join(format!("bootstrap-{}", history.name()));
            let result = database
                .synchronize_tables_with_stage_directory_candidate_validated_checked(
                    transport,
                    &SessionConfig {
                        direction,
                        ..SessionConfig::default()
                    },
                    if direction == SyncDirection::ReceiveOnly {
                        SyncRole::Initiator
                    } else {
                        SyncRole::Responder
                    },
                    directory,
                    &tables,
                    |table, column, value| {
                        history.validate_value(table, column, value, &self.installation)
                    },
                    |view| {
                        if (direction == SyncDirection::ReceiveOnly
                            && view.direction == SyncValidationDirection::Outgoing)
                            || (direction == SyncDirection::SendOnly
                                && view.direction == SyncValidationDirection::Incoming)
                        {
                            return Err(
                                "bootstrap reverse application history is prohibited".into()
                            );
                        }
                        history.validate_candidate(
                            view,
                            &self.installation,
                            &[PeerBinding {
                                member: approval.request.member_id,
                                endpoint: approval.request.endpoint_id.clone(),
                                owner: approval.owner_subject,
                            }],
                            Some(&candidate),
                        )
                    },
                    &mut check,
                )
                .await
                .map_err(|error| match error {
                    SyncError::PreApply(reason) => denied(reason),
                    SyncError::Transport(error) => error,
                    error => io::Error::other(error.to_string()),
                })?;
            super::accumulate(&mut total, result);
        }
        Ok(total)
    }

    async fn verify_node_join_public_readiness(
        &self,
        owner: &impl NodeJoinOwner,
    ) -> io::Result<()> {
        let approval = owner.node_join_anchor()?;
        if self.cancellation.is_cancelled()
            || !Arc::ptr_eq(&self.idp, &owner.node_join_database()?)
            || approval.request.installation != self.installation
            || approval.request.endpoint_id != self.server.endpoint().id().to_string()
            || !owner.node_join_stage_recorded(NodeJoinStage::Idp)?
            || !owner.node_join_stage_recorded(NodeJoinStage::Management)?
        {
            return Err(denied("both durable public context imports are required"));
        }
        bootstrap_stage(|| self.check_join_local_binding(owner))
            .await
            .map_err(denied)?;
        self.check_exact_join_trust(&approval).await?;
        LocalIdentity::check_configured_join_signer(
            self.binding.as_ref(),
            approval.request.member_id,
            &approval.request.endpoint_id,
            &approval.request.public_signer,
        )?;
        self.check_join_context_grants(&approval).await
    }

    pub async fn check_join_context_grants(&self, approval: &NodeJoinApproval) -> io::Result<()> {
        self.check_public_context_grants(approval.owner_subject)
            .await
    }

    async fn check_public_context_grants(&self, owner: Id) -> io::Result<()> {
        for context in [SecurityContext::Idp, SecurityContext::Device] {
            self.owner().check_permission(owner, context).await?;
        }
        Ok(())
    }

    /// Completes public IdP/Management bootstrap, not Storage readiness or resource authorization.
    pub async fn complete_node_join_public_bootstrap(
        &self,
        owner: &impl NodeJoinOwner,
    ) -> io::Result<()> {
        self.verify_node_join_public_readiness(owner).await?;
        owner.record_node_join_stage(NodeJoinStage::PublicReady)?;
        Ok(())
    }

    pub async fn local_node_public_ready(&self, owner: &impl NodeJoinOwner) -> io::Result<bool> {
        if owner.node_join_request()?.is_some() || owner.node_join_approval()?.is_some() {
            return self.node_join_public_ready(owner).await;
        }
        let check = async {
            if self.cancellation.is_cancelled()
                || !Arc::ptr_eq(&self.idp, &owner.node_join_database()?)
            {
                return Err(denied("current shared owner runtime is required"));
            }
            let binding = self
                .binding
                .as_ref()
                .ok_or_else(|| denied("local signer binding is required"))?;
            owner.verify_node_signer(binding).await?;
            for context in [SecurityContext::Idp, SecurityContext::Device] {
                let local = self.local_handshake(context).await?;
                if local.member != binding.member_id {
                    return Err(denied("local member binding changed"));
                }
            }
            let device = DbDeviceRepo::new(self.management.clone().client())
                .find_approved_by_public_key(&self.server.endpoint().id().to_string())
                .await
                .map_err(denied)?
                .ok_or_else(|| denied("local Device is not approved"))?;
            let subject: Id = device.owner_subject.parse().map_err(denied)?;
            if device.id != binding.member_id
                || !LocalIdentity::new(self.idp.client())
                    .is_active_user(subject)
                    .await?
            {
                return Err(denied("local Device requires its active owner"));
            }
            self.check_public_context_grants(subject).await
        }
        .await;
        Ok(check.is_ok())
    }

    pub async fn node_join_public_ready(&self, owner: &impl NodeJoinOwner) -> io::Result<bool> {
        if owner.node_join_approval()?.is_none()
            || !owner.node_join_stage_recorded(NodeJoinStage::PublicReady)?
        {
            return Ok(false);
        }
        Ok(self.verify_node_join_public_readiness(owner).await.is_ok())
    }
}

struct NodeBootstrapHandler {
    protocol: SecurityProtocol,
}

impl std::fmt::Debug for NodeBootstrapHandler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NodeBootstrapHandler")
    }
}

impl iroh::protocol::ProtocolHandler for NodeBootstrapHandler {
    async fn accept(&self, connection: Connection) -> Result<(), iroh::protocol::AcceptError> {
        let mut tasks = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                _ = self.protocol.cancellation.cancelled() => break,
                result = tasks.join_next(), if !tasks.is_empty() => {
                    #[cfg(test)]
                    match &result {
                        Some(Ok(Err(error))) => eprintln!("bootstrap source stage=stream error={error}"),
                        Some(Err(error)) => eprintln!("bootstrap source stage=task error={error}"),
                        _ => {}
                    }
                    let _ = result;
                },
                streams = connection.accept_bi() => {
                    let Ok((send, mut recv)) = streams else { break; };
                    let protocol = self.protocol.clone();
                    let connection = connection.clone();
                    tasks.spawn(async move {
                        let mut kind = [0u8; 1];
                        recv.read_exact(&mut kind).await.map_err(io::Error::other)?;
                        if kind[0] != BOOTSTRAP_STREAM_KIND { return Err(denied("only purpose-specific bootstrap streams are allowed")); }
                        bootstrap_stage(|| protocol.accept_node_bootstrap(connection, send, recv)).await
                    });
                }
            }
        }
        tasks.shutdown().await;
        Ok(())
    }
}

fn stage(context: SecurityContext) -> NodeJoinStage {
    match context {
        SecurityContext::Idp => NodeJoinStage::Idp,
        SecurityContext::Device => NodeJoinStage::Management,
    }
}

struct SourceTransport<'a, T> {
    inner: &'a mut T,
    protocol: &'a SecurityProtocol,
    connection: &'a Connection,
    approval: &'a NodeJoinApproval,
    context: SecurityContext,
    #[cfg(test)]
    last_send: std::time::Instant,
}
impl<T: SyncTransport<Error = io::Error>> SyncTransport for SourceTransport<'_, T> {
    type Error = io::Error;
    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        #[cfg(test)]
        let started = std::time::Instant::now();
        self.protocol
            .authorize_bootstrap_source(self.connection, self.approval, self.context)
            .await?;
        #[cfg(test)]
        eprintln!(
            "bootstrap source stage=outgoing_authorization context={:?} elapsed_ms={} interval_ms={} frame_bytes={}",
            self.context,
            started.elapsed().as_millis(),
            self.last_send.elapsed().as_millis(),
            frame.len()
        );
        #[cfg(test)]
        {
            self.last_send = std::time::Instant::now();
        }
        self.inner.send(frame).await
    }
    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        self.inner.receive().await
    }
}

struct ReceiverTransport<'a, T, O: NodeJoinOwner> {
    inner: &'a mut T,
    protocol: &'a SecurityProtocol,
    connection: &'a Connection,
    owner: &'a O,
    approval: &'a NodeJoinApproval,
}
impl<T: SyncTransport<Error = io::Error>, O: NodeJoinOwner> SyncTransport
    for ReceiverTransport<'_, T, O>
{
    type Error = io::Error;
    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        bootstrap_stage(|| {
            self.protocol
                .check_bootstrap_receiver(self.connection, self.owner, self.approval)
        })
        .await?;
        self.inner.send(frame).await
    }
    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        self.inner.receive().await
    }
}

#[inline(never)]
fn bootstrap_stage<'a, T: 'a, F>(
    make: impl FnOnce() -> F,
) -> Pin<Box<dyn Future<Output = io::Result<T>> + Send + 'a>>
where
    F: Future<Output = io::Result<T>> + Send + 'a,
{
    Box::pin(make())
}
