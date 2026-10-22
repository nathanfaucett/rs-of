use std::{io, path::Path, sync::Arc};

use idp_server::{
    OwnerProvisioner,
    bootstrap_host::{
        BootstrapEndpoint, BootstrapHost, BootstrapOwners, BootstrapProtocol, BootstrapRuntime,
    },
};
use idp_service::oauth2::NodeSignerBinding;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::{
    LocalTls, RuntimeOwners, SecurityProtocol, UnifiedConfig, UnifiedEndpoint, UnifiedRuntime,
};

pub struct UnifiedBootstrapHost;

pub type BootstrapConfig = idp_server::BootstrapConfig<UnifiedBootstrapHost>;
pub type CredentialBootstrap = idp_server::CredentialBootstrap<UnifiedBootstrapHost>;
pub type LocalProvisioner = idp_server::LocalProvisioner<UnifiedBootstrapHost>;
pub type LocalJoin = idp_server::LocalJoin<UnifiedBootstrapHost>;

impl BootstrapHost for UnifiedBootstrapHost {
    type Config = UnifiedConfig;
    type Endpoint = UnifiedEndpoint;
    type Runtime = UnifiedRuntime;
    type Protocol = SecurityProtocol;

    async fn open_owners(config: &UnifiedConfig) -> io::Result<BootstrapOwners<UnifiedEndpoint>> {
        let owners = RuntimeOwners::open(config).await?;
        Ok(BootstrapOwners {
            endpoint: owners.endpoint,
            idp: owners.idp,
            management: owners.management,
        })
    }
}

impl BootstrapEndpoint for UnifiedEndpoint {
    async fn open(path: &Path, config: &idp_server::AppConfig) -> io::Result<Self> {
        Self::open(path, config).await
    }
    async fn open_fresh(path: &Path, config: &idp_server::AppConfig) -> io::Result<Self> {
        Self::open_fresh(path, config).await
    }
    fn persisted_endpoint_id(path: &Path) -> io::Result<iroh::EndpointId> {
        Self::persisted_endpoint_id(path)
    }
    fn endpoint_id(&self) -> iroh::EndpointId {
        self.endpoint_id()
    }
    async fn close(&self) {
        self.close().await
    }
}

impl BootstrapRuntime<UnifiedConfig, UnifiedEndpoint> for UnifiedRuntime {
    async fn build(config: UnifiedConfig) -> io::Result<Self> {
        Self::build(config).await
    }
    async fn build_https(config: UnifiedConfig) -> io::Result<Self> {
        Self::build_https(config).await
    }
    async fn build_hosted_https(config: UnifiedConfig) -> io::Result<Self> {
        Self::build_hosted_https(config).await
    }
    async fn build_with_owners(
        config: UnifiedConfig,
        listener: Option<TcpListener>,
        api_base: String,
        owners: BootstrapOwners<UnifiedEndpoint>,
    ) -> io::Result<Self> {
        Self::build_with_owners(
            config,
            listener,
            api_base,
            RuntimeOwners {
                endpoint: owners.endpoint,
                idp: owners.idp,
                management: owners.management,
            },
        )
        .await
    }
    fn endpoint_id(&self) -> iroh::EndpointId {
        self.endpoint_id()
    }
    fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token()
    }
    fn join_owner(&self) -> &OwnerProvisioner {
        self.join_owner()
    }
    async fn node_public_ready(&self) -> io::Result<bool> {
        self.node_public_ready().await
    }
    async fn node_join_public_ready(&self) -> io::Result<bool> {
        self.node_join_public_ready().await
    }
    async fn start_background_tasks(&mut self) -> io::Result<()> {
        self.start_background_tasks().await
    }
    async fn verify_local_bootstrap(self, tls: Option<&LocalTls>) -> io::Result<()> {
        self.verify_local_bootstrap(tls).await
    }
    async fn shutdown(self) -> io::Result<()> {
        self.shutdown().await
    }
}

impl BootstrapProtocol<UnifiedEndpoint> for SecurityProtocol {
    async fn open_node_bootstrap(
        endpoint: &UnifiedEndpoint,
        config: &idp_server::AppConfig,
        directory: &Path,
        binding: NodeSignerBinding,
    ) -> io::Result<(Self, OwnerProvisioner)> {
        Self::open_node_bootstrap(endpoint, config, directory, binding).await
    }
    async fn join_from_trusted_peer(
        &self,
        owner: &OwnerProvisioner,
        name: &str,
        peer: &str,
    ) -> io::Result<management_service::hosted_join::NodeJoinApproval> {
        self.join_from_trusted_peer(owner, name, peer).await
    }
    fn into_runtime_owners(
        self,
        endpoint: Arc<UnifiedEndpoint>,
    ) -> BootstrapOwners<UnifiedEndpoint> {
        let owners = self.into_runtime_owners(endpoint);
        BootstrapOwners {
            endpoint: owners.endpoint,
            idp: owners.idp,
            management: owners.management,
        }
    }
}

pub(crate) async fn discover_with_cancellation(
    peer: &str,
    cancellation: &CancellationToken,
) -> io::Result<String> {
    #[cfg(test)]
    let preset = crate::endpoint::TestPreset;
    #[cfg(not(test))]
    let preset = iroh::endpoint::presets::N0;
    idp_server::issuer_discovery::discover_with_preset(peer, cancellation, preset).await
}

pub async fn discover_installation_issuer(peer: &str) -> io::Result<String> {
    discover_with_cancellation(peer, &CancellationToken::new()).await
}
