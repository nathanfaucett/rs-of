use db::DatabaseTransport;
use db::SyncTransport as OldSyncTransport;
use idp_service::replica::LocalIdentity;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use of_model::idp::model::Id;
use std::{future::Future, io, sync::Arc};

use ofdb_sync::{
    IrohTransport, Message, SyncResult, SyncTransport, SynchronizeConfig,
    SynchronizeError as SyncError,
};
use serde::{Deserialize, Serialize};

use super::TransportRef;
use crate::DeviceRepo;
use crate::hosted_join::NodeJoinApproval;
use crate::node_join_owner::{NodeJoinOwner, NodeJoinStage};
use crate::replica::DbDeviceRepo;

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

        let (send, recv) = transport.into_streams();
        let framed = IrohTransport::new(send, recv);
        let result = bootstrap_stage(|| {
            self.receive_bootstrap_history(
                connection,
                owner,
                &approval,
                context,
                framed,
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

    pub async fn accept_node_bootstrap(
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

        let (send, recv) = transport.into_streams();
        let mut framed = IrohTransport::new(send, recv);
        let mut source_transport = SourceTransport {
            transport: &mut framed,
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
                &mut source_transport,
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

    async fn check_join_identity(&self, approval: &NodeJoinApproval) -> io::Result<()> {
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

    pub async fn check_exact_join_trust(&self, approval: &NodeJoinApproval) -> io::Result<()> {
        self.check_join_identity(approval).await?;
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
        Ok(())
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
        Ok(())
    }

    pub async fn check_join_context_grants(&self, approval: &NodeJoinApproval) -> io::Result<()> {
        self.check_exact_join_trust(approval).await?;
        let request = &approval.request;
        let device = DbDeviceRepo::new(self.management.clone().client())
            .find_approved_by_public_key(&request.endpoint_id)
            .await
            .map_err(denied)?
            .ok_or_else(|| denied("bootstrap requires exact approved Device"))?;
        let owner = device.owner_subject.parse().map_err(denied)?;
        self.owner()
            .check_permission(owner, SecurityContext::Device)
            .await
    }

    pub async fn check_public_context_grants(&self, approval: &NodeJoinApproval) -> io::Result<()> {
        let request = &approval.request;
        let device = DbDeviceRepo::new(self.management.clone().client())
            .find_approved_by_public_key(&request.endpoint_id)
            .await
            .map_err(denied)?
            .ok_or_else(|| denied("bootstrap requires exact approved Device"))?;
        let owner = device.owner_subject.parse().map_err(denied)?;
        for context in [SecurityContext::Idp, SecurityContext::Device] {
            self.owner().check_permission(owner, context).await?;
        }
        Ok(())
    }

    pub async fn complete_node_join_public_bootstrap(
        &self,
        owner: &impl NodeJoinOwner,
    ) -> io::Result<()> {
        let approval = owner.node_join_anchor()?;
        let request = &approval.request;
        if self.node_join_public_ready(owner).await? {
            return Ok(());
        }
        for peer in self.server.peers().ids() {
            if peer.to_string() == request.approving_peer {
                continue;
            }
            let connection = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                self.server.endpoint().connect(peer, BOOTSTRAP_ALPN),
            )
            .await
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "public bootstrap peer connection timed out",
                )
            })?
            .map_err(io::Error::other)?;
            let (send, recv) = connection.open_bi().await.map_err(io::Error::other)?;
            let mut transport = DatabaseTransport::new(send, recv);
            transport.send(serde_json::to_vec(&request)?).await?;
            let reply =
                tokio::time::timeout(std::time::Duration::from_secs(10), transport.receive())
                    .await
                    .map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::TimedOut,
                            "public bootstrap peer acknowledgment timed out",
                        )
                    })??;
            let _accepted: BootstrapRequest = serde_json::from_slice(&reply)?;
            connection.close(0u32.into(), b"public bootstrap finished");
        }
        Ok(())
    }

    pub async fn local_node_public_ready(&self, owner: &impl NodeJoinOwner) -> io::Result<bool> {
        self.verify_node_join_public_readiness(owner).await
    }

    pub async fn node_join_public_ready(&self, owner: &impl NodeJoinOwner) -> io::Result<bool> {
        self.verify_node_join_public_readiness(owner).await
    }

    async fn receive_bootstrap_history<Q, U>(
        &self,
        connection: &Connection,
        owner: &impl NodeJoinOwner,
        approval: &NodeJoinApproval,
        context: SecurityContext,
        mut transport: IrohTransport,
        mut check: Q,
    ) -> io::Result<SyncResult>
    where
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
        let mut auth_transport = ReceiverTransport {
            transport: &mut transport,
            protocol: self,
            connection,
            owner,
            approval,
        };
        self.node_bootstrap_session(approval, context, &mut auth_transport, || {
            let additional = check();
            async move {
                bootstrap_stage(|| self.check_bootstrap_receiver(connection, owner, approval))
                    .await
                    .map_err(|_| "bootstrap receiver no longer authorized".to_owned())?;
                additional.await
            }
        })
        .await
    }

    async fn node_bootstrap_session<T, Q, U>(
        &self,
        _approval: &NodeJoinApproval,
        context: SecurityContext,
        transport: &mut T,
        mut check: Q,
    ) -> io::Result<SyncResult>
    where
        T: SyncTransport<Error = io::Error> + Send,
        Q: FnMut() -> U + Send,
        U: Future<Output = Result<(), String>> + Send,
    {
        let mut total = SyncResult::default();
        for (database, _history, _tables) in self.sessions(context) {
            check().await.map_err(denied)?;
            let result = database
                .synchronize(TransportRef(transport), SynchronizeConfig::default())
                .await
                .map_err(|error| match error {
                    SyncError::Apply(reason) => denied(reason),
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
    ) -> io::Result<bool> {
        let approval = owner.node_join_anchor()?;
        let request = &approval.request;
        let device = DbDeviceRepo::new(self.management.clone().client())
            .find_approved_by_public_key(&request.endpoint_id)
            .await
            .map_err(denied)?
            .ok_or_else(|| denied("bootstrap requires exact approved Device"))?;
        let actor = device.owner_subject.parse().map_err(denied)?;
        for context in [SecurityContext::Idp, SecurityContext::Device] {
            if let Err(error) = self.owner().check_permission(actor, context).await {
                if error.kind() == io::ErrorKind::PermissionDenied {
                    return Ok(false);
                }
                return Err(error);
            }
        }
        Ok(true)
    }
}

fn stage(context: SecurityContext) -> NodeJoinStage {
    match context {
        SecurityContext::Idp => NodeJoinStage::Idp,
        SecurityContext::Device => NodeJoinStage::Management,
    }
}

async fn bootstrap_stage<F, Fut, T, E>(f: F) -> io::Result<T>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    tokio::select! {
        biased;
        result = f() => result.map_err(|e| io::Error::other(e.to_string())),
        _ = tokio::signal::ctrl_c() => Err(io::Error::other("cancelled")),
    }
}

struct SourceTransport<'a> {
    transport: &'a mut IrohTransport,
    protocol: &'a SecurityProtocol,
    connection: &'a Connection,
    approval: &'a NodeJoinApproval,
    context: SecurityContext,
    #[cfg(test)]
    last_send: std::time::Instant,
}

impl SyncTransport for SourceTransport<'_> {
    type Error = io::Error;

    async fn send(&mut self, msg: Message) -> io::Result<()> {
        self.protocol
            .authorize_bootstrap_source(self.connection, self.approval, self.context)
            .await?;
        #[cfg(test)]
        {
            let elapsed = self.last_send.elapsed().as_millis();
            if elapsed > 1000 {
                eprintln!("bootstrap source send latency: {elapsed}ms");
            }
            self.last_send = std::time::Instant::now();
        }
        self.transport.send(msg).await.map_err(io::Error::other)
    }

    async fn receive(&mut self) -> io::Result<Message> {
        self.transport.receive().await.map_err(io::Error::other)
    }
}

struct ReceiverTransport<'a, O> {
    transport: &'a mut IrohTransport,
    protocol: &'a SecurityProtocol,
    connection: &'a Connection,
    owner: &'a O,
    approval: &'a NodeJoinApproval,
}

impl<O: NodeJoinOwner> SyncTransport for ReceiverTransport<'_, O> {
    type Error = io::Error;

    async fn send(&mut self, msg: Message) -> io::Result<()> {
        self.protocol
            .check_bootstrap_receiver(self.connection, self.owner, self.approval)
            .await?;
        self.transport.send(msg).await.map_err(io::Error::other)
    }

    async fn receive(&mut self) -> io::Result<Message> {
        self.transport.receive().await.map_err(io::Error::other)
    }
}

struct NodeBootstrapHandler {
    protocol: SecurityProtocol,
}

impl std::fmt::Debug for NodeBootstrapHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeBootstrapHandler").finish()
    }
}

impl iroh::protocol::ProtocolHandler for NodeBootstrapHandler {
    async fn accept(&self, connection: Connection) -> Result<(), iroh::protocol::AcceptError> {
        let (send, recv) = connection.accept_bi().await.map_err(io::Error::other)?;
        self.protocol
            .accept_node_bootstrap(connection, send, recv)
            .await
            .map_err(iroh::protocol::AcceptError::from_err)
    }
}
