use std::{future::Future, io, path::Path, sync::Arc};

use device_service::DeviceRepo;
use device_service::hosted_join::NodeJoinApproval;
use device_service::replica::DbDeviceRepo;
use idp_model::{
    contract::{
        IdpSignerRecord, PermissionEvaluationRequest, PermissionSubject, PermissionTarget,
        ReplicaMembership,
    },
    model::Id,
};
use idp_server::{NodeJoinStage, OwnerProvisioner};
use idp_service::{
    PasswordConfig,
    oauth2::NodeSignerBinding,
    replica::{DbReplicaSignerRepo, DbUserRepo},
    repo::{PrivateKeyRepo, UserRepo},
};
use iroh::endpoint::{Connection, RecvStream, SendStream};
use management_service::LocalPermissions;
use ofdb_sql::{
    SessionConfig, SyncDirection, SyncError, SyncResult, SyncRole, SyncTransport,
    SyncValidationDirection,
};
use serde::{Deserialize, Serialize};
use storage_server::DatabaseTransport;

use idp_server::local_provisioning::provisioning_stage;

use super::{SecurityContext, SecurityProtocol, candidates, denied, now};

#[cfg(test)]
#[path = "node_bootstrap_interruption_tests.rs"]
mod interruption_tests;

pub(super) const BOOTSTRAP_STREAM_KIND: u8 = 4;
const PURPOSE: &str = "of.equal-node-public-bootstrap.v2";

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct BootstrapRequest {
    purpose: String,
    context: SecurityContext,
    approval: NodeJoinApproval,
}

impl SecurityProtocol {
    pub fn into_runtime_owners(
        self,
        endpoint: Arc<crate::UnifiedEndpoint>,
    ) -> crate::runtime::RuntimeOwners {
        crate::runtime::RuntimeOwners {
            endpoint,
            idp: self.idp,
            devices: self.management,
        }
    }

    pub async fn open_node_bootstrap(
        endpoint: &crate::UnifiedEndpoint,
        config: &idp_server::AppConfig,
        management_directory: &Path,
        binding: NodeSignerBinding,
    ) -> io::Result<(Self, OwnerProvisioner)> {
        if binding.endpoint_id != endpoint.endpoint_id().to_string() {
            return Err(denied(
                "bootstrap signer must bind the shared local endpoint",
            ));
        }
        let owner = OwnerProvisioner::open(config).await?;
        std::fs::create_dir_all(management_directory)?;
        let management = Arc::new(
            db::Database::open(management_directory.join("devices.redb"))
                .map_err(io::Error::other)?,
        );
        device_service::replica::up(&management)
            .await
            .map_err(io::Error::other)?;
        let protocol = Self::new(
            endpoint.server().clone(),
            owner.node_join_database()?,
            management,
            owner.node_join_database()?,
            config.oauth2.issuer.clone(),
            Some(binding),
            management_directory.join("node-bootstrap-staging"),
            tokio_util::sync::CancellationToken::new(),
        );
        Ok((protocol, owner))
    }

    /// Drives only purpose-bound public bootstrap; the host must verify all local services separately.
    pub async fn join_from_trusted_peer(
        &self,
        owner: &OwnerProvisioner,
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
        provisioning_stage(|| self.check_join_local_binding(owner)).await?;
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
            provisioning_stage(|| self.check_join_local_binding(owner)).await?;
            if owner.node_join_anchor()? != approval {
                return Err(denied("join anchor changed during admission refresh"));
            }
            let attempt = async {
                let connection = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    self.server
                        .endpoint()
                        .connect(peer, idp_server::BOOTSTRAP_ALPN),
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
                            provisioning_stage(|| {
                                self.receive_node_bootstrap(&connection, owner, context)
                            })
                            .await
                            .inspect_err(|_error| {
                                #[cfg(test)]
                                eprintln!("bootstrap receiver stage=receive context={context:?} error={_error}");
                            })?;
                        }
                    }
                    provisioning_stage(|| self.complete_node_join_public_bootstrap(owner))
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

    async fn check_join_local_binding<P: PrivateKeyRepo + Send + Sync>(
        &self,
        owner: &OwnerProvisioner<P>,
    ) -> io::Result<()> {
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
        let users = DbUserRepo::new(self.idp.client(), PasswordConfig::default());
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
            if owner.is_nil()
                || users
                    .find_user_by_id(owner)
                    .await
                    .map_err(denied)?
                    .is_none()
            {
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

    pub async fn receive_node_bootstrap<P: PrivateKeyRepo + Send + Sync>(
        &self,
        connection: &Connection,
        owner: &OwnerProvisioner<P>,
        context: SecurityContext,
    ) -> io::Result<SyncResult> {
        #[cfg(test)]
        let started = std::time::Instant::now();
        let approval = owner.node_join_anchor()?;
        provisioning_stage(|| self.check_bootstrap_receiver(connection, owner, &approval)).await?;
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
        provisioning_stage(|| self.check_bootstrap_receiver(connection, owner, &approval)).await?;
        send.write_all(&[BOOTSTRAP_STREAM_KIND])
            .await
            .map_err(io::Error::other)?;
        let mut transport = DatabaseTransport::new(send, recv);
        provisioning_stage(|| self.check_bootstrap_receiver(connection, owner, &approval)).await?;
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
        let mut transport = ReceiverTransport {
            inner: &mut transport,
            protocol: self,
            connection,
            owner,
            approval: &approval,
        };
        let result = provisioning_stage(|| {
            self.node_bootstrap_session(
                &approval,
                context,
                SyncDirection::ReceiveOnly,
                &mut transport,
                || async {
                    #[cfg(test)]
                    let check_started = std::time::Instant::now();
                    let result = provisioning_stage(|| {
                        self.check_bootstrap_receiver(connection, owner, &approval)
                    })
                    .await
                    .map_err(|_| "bootstrap receiver no longer authorized".into());
                    #[cfg(test)]
                    eprintln!(
                        "bootstrap receiver stage=pre_apply context={context:?} elapsed_ms={}",
                        check_started.elapsed().as_millis()
                    );
                    result
                },
            )
        })
        .await?;
        provisioning_stage(|| self.check_bootstrap_receiver(connection, owner, &approval)).await?;
        #[cfg(test)]
        interruption_tests::interrupt(match context {
            SecurityContext::Idp => interruption_tests::Transition::IdpImported,
            SecurityContext::Device => interruption_tests::Transition::ManagementImported,
        })?;
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
        provisioning_stage(|| {
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
        if connection.alpn() != idp_server::BOOTSTRAP_ALPN {
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
        let trusted = self.check_exact_join_trust(approval).await?;
        let local = self
            .binding
            .as_ref()
            .ok_or_else(|| denied("local node signer is not configured"))?;
        let endpoint = self.server.endpoint().id().to_string();
        if local.endpoint_id != endpoint {
            return Err(denied("local node signer/member is not currently trusted"));
        }
        if !trusted.iter().any(|(member, signer)| {
            member.member_id == local.member_id
                && member.endpoint_id == endpoint
                && signer.key_id == local.key_id
        }) {
            return Err(denied("local node signer/member is not currently trusted"));
        }
        let device = DbDeviceRepo::new(self.management.clone().client())
            .find_approved_by_public_key(&endpoint)
            .await
            .map_err(denied)?
            .ok_or_else(|| denied("bootstrap source has no approved Device"))?;
        let (action, resource) = context.permission();
        if approval.owner_subject.is_nil()
            || !LocalPermissions::new(self.policy.client())
                .evaluate(&PermissionEvaluationRequest {
                    request_id: Id::now_v7(),
                    subject: PermissionSubject::User {
                        id: approval.owner_subject,
                    },
                    action,
                    target: PermissionTarget::Installation { resource },
                })
                .await
                .map_err(denied)?
        {
            return Err(denied(
                "local actor lacks this context's security sync permission",
            ));
        }

        let owner: Id = device.owner_subject.parse().map_err(denied)?;
        if device.id != local.member_id
            || owner.is_nil()
            || DbUserRepo::new(self.idp.client(), PasswordConfig::default())
                .find_user_by_id(owner)
                .await
                .map_err(denied)?
                .is_none()
        {
            return Err(denied(
                "bootstrap source has no active conflict-clear Device owner",
            ));
        }
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
        let key: Id = request.public_signer.kid.parse().map_err(denied)?;
        let trusted = DbReplicaSignerRepo::new(self.idp.client())
            .trusted_signers(&self.installation, now()?)
            .await
            .map_err(denied)?;
        let mut matching = trusted.iter().filter(|(member, signer)| {
            member.member_id == request.member_id
                && member.endpoint_id == request.endpoint_id
                && signer.key_id == key
                && signer.public_jwk == request.public_signer
        });
        matching.next().ok_or_else(|| {
            denied("bootstrap requires exact current enrolled member and independent signer")
        })?;
        if matching.next().is_some() {
            return Err(denied(
                "bootstrap requires exact current enrolled member and independent signer",
            ));
        }
        if DbUserRepo::new(self.idp.client(), PasswordConfig::default())
            .find_user_by_id(approval.owner_subject)
            .await
            .map_err(denied)?
            .is_none()
        {
            return Err(denied(
                "bootstrap owner is not a current conflict-clear User",
            ));
        }
        Ok(trusted)
    }

    async fn check_exact_join_trust(
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

    async fn check_bootstrap_receiver<P: PrivateKeyRepo + Send + Sync>(
        &self,
        connection: &Connection,
        owner: &OwnerProvisioner<P>,
        approval: &NodeJoinApproval,
    ) -> io::Result<()> {
        if connection.alpn() != idp_server::BOOTSTRAP_ALPN
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
        provisioning_stage(|| self.check_join_local_binding(owner))
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
        let candidate = candidates::JoinCandidate {
            member_id: approval.request.member_id,
            device_id: approval.request.member_id,
            endpoint: &approval.request.endpoint_id,
            owner: approval.owner_subject,
            public_signer: &approval.request.public_signer,
            name: &approval.request.name,
        };
        let mut total = SyncResult::default();
        for (database, name, tables) in self.sessions(context) {
            let directory = self.staging_directory.join(format!("bootstrap-{name}"));
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
                        context.validate_value(table, column, value)?;
                        if context == SecurityContext::Idp
                            && !candidates::permits_installation(
                                table,
                                column,
                                value,
                                &self.installation,
                            )
                        {
                            return Err("invalid public security historical value".into());
                        }
                        Ok(())
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
                        match context {
                            SecurityContext::Idp => {
                                candidates::validate_node_join(view, &self.installation, &candidate)
                            }
                            SecurityContext::Device if name == "policy-identity" => {
                                candidates::validate(view, &self.installation, &[])
                            }
                            SecurityContext::Device => candidates::validate_device_history(
                                view,
                                &[candidates::PeerBinding {
                                    member: approval.request.member_id,
                                    endpoint: approval.request.endpoint_id.clone(),
                                    owner: approval.owner_subject,
                                }],
                                Some(&candidate),
                            ),
                        }
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

    async fn verify_node_join_public_readiness<P: PrivateKeyRepo + Send + Sync>(
        &self,
        owner: &OwnerProvisioner<P>,
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
        provisioning_stage(|| self.check_join_local_binding(owner))
            .await
            .map_err(denied)?;
        self.check_exact_join_trust(&approval).await?;
        let binding = NodeSignerBinding {
            member_id: approval.request.member_id,
            key_id: approval.request.public_signer.kid.parse().map_err(denied)?,
            endpoint_id: approval.request.endpoint_id.clone(),
        };
        if !self.binding.as_ref().is_some_and(|configured| {
            configured.member_id == binding.member_id
                && configured.key_id == binding.key_id
                && configured.endpoint_id == binding.endpoint_id
        }) {
            return Err(denied("local independent signing capability is not ready"));
        }
        self.check_join_context_grants(&approval).await
    }

    async fn check_join_context_grants(&self, approval: &NodeJoinApproval) -> io::Result<()> {
        self.check_public_context_grants(approval.owner_subject)
            .await
    }

    async fn check_public_context_grants(&self, owner: Id) -> io::Result<()> {
        for context in [SecurityContext::Idp, SecurityContext::Device] {
            let (action, resource) = context.permission();
            if !LocalPermissions::new(self.policy.client())
                .evaluate(&PermissionEvaluationRequest {
                    request_id: Id::now_v7(),
                    subject: PermissionSubject::User { id: owner },
                    action,
                    target: PermissionTarget::Installation { resource },
                })
                .await
                .map_err(denied)?
            {
                return Err(denied("local owner lacks exact public context permission"));
            }
        }
        Ok(())
    }

    /// Completes public IdP/Management bootstrap, not Storage readiness or resource authorization.
    pub async fn complete_node_join_public_bootstrap<P: PrivateKeyRepo + Send + Sync>(
        &self,
        owner: &OwnerProvisioner<P>,
    ) -> io::Result<()> {
        self.verify_node_join_public_readiness(owner).await?;
        #[cfg(test)]
        interruption_tests::interrupt(interruption_tests::Transition::ReadinessVerified)?;
        owner.record_node_join_stage(NodeJoinStage::PublicReady)?;
        #[cfg(test)]
        interruption_tests::interrupt(interruption_tests::Transition::PublicReadyPublished)?;
        Ok(())
    }

    pub async fn local_node_public_ready<P: PrivateKeyRepo + Send + Sync>(
        &self,
        owner: &OwnerProvisioner<P>,
    ) -> io::Result<bool> {
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
                || subject.is_nil()
                || DbUserRepo::new(self.idp.client(), PasswordConfig::default())
                    .find_user_by_id(subject)
                    .await
                    .map_err(denied)?
                    .is_none()
            {
                return Err(denied("local Device requires its active owner"));
            }
            self.check_public_context_grants(subject).await
        }
        .await;
        Ok(check.is_ok())
    }

    pub async fn node_join_public_ready<P: PrivateKeyRepo + Send + Sync>(
        &self,
        owner: &OwnerProvisioner<P>,
    ) -> io::Result<bool> {
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
                        provisioning_stage(|| protocol.accept_node_bootstrap(connection, send, recv)).await
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

struct ReceiverTransport<'a, T, P: PrivateKeyRepo + Send + Sync> {
    inner: &'a mut T,
    protocol: &'a SecurityProtocol,
    connection: &'a Connection,
    owner: &'a OwnerProvisioner<P>,
    approval: &'a NodeJoinApproval,
}
impl<T: SyncTransport<Error = io::Error>, P: PrivateKeyRepo + Send + Sync> SyncTransport
    for ReceiverTransport<'_, T, P>
{
    type Error = io::Error;
    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        provisioning_stage(|| {
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
