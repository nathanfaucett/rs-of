use std::{future::Future, io, path::Path, sync::Arc};

use api::{LocalTls, LocalTlsConfig};
use db::Database;
use idp_service::oauth2::NodeSignerBinding;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::{AppConfig, BootstrapSettings, OwnerProvisioner};

pub struct BootstrapOwners<E> {
    pub endpoint: Arc<E>,
    pub idp: Arc<Database>,
    pub management: Arc<Database>,
}

/// Host capabilities only; retained setup decisions and journals belong to IdP.
pub trait BootstrapHost: Send + Sync + 'static {
    type Config: Clone + Send + Sync + LocalTlsConfig;
    type Endpoint: BootstrapEndpoint;
    type Runtime: BootstrapRuntime<Self::Config, Self::Endpoint>;
    type Protocol: BootstrapProtocol<Self::Endpoint>;

    fn settings(config: &Self::Config) -> BootstrapSettings;
    fn configure(config: &Self::Config, settings: BootstrapSettings) -> Self::Config;
    fn from_settings(settings: BootstrapSettings) -> Self::Config;
    fn open_management(
        config: &Self::Config,
    ) -> impl Future<Output = io::Result<Arc<Database>>> + Send;
    fn provision_administrator(
        database: Arc<Database>,
        user: idp_model::model::Id,
        role: idp_model::model::Id,
        permissions: &[(idp_model::model::Id, idp_model::contract::IdentityAction)],
    ) -> impl Future<Output = io::Result<()>> + Send;
    fn provision_device(
        database: Arc<Database>,
        member: idp_model::model::Id,
        user: idp_model::model::Id,
        endpoint: &str,
        name: &str,
    ) -> impl Future<Output = io::Result<()>> + Send;

    fn open_owners(
        config: &Self::Config,
    ) -> impl Future<Output = io::Result<BootstrapOwners<Self::Endpoint>>> + Send;
}

pub trait BootstrapEndpoint: Send + Sync + Sized + 'static {
    fn open(data_dir: &Path, config: &AppConfig) -> impl Future<Output = io::Result<Self>> + Send;
    fn open_fresh(
        data_dir: &Path,
        config: &AppConfig,
    ) -> impl Future<Output = io::Result<Self>> + Send;
    fn persisted_endpoint_id(data_dir: &Path) -> io::Result<iroh::EndpointId>;
    fn endpoint_id(&self) -> iroh::EndpointId;
    fn close(&self) -> impl Future<Output = ()> + Send;
}

pub trait BootstrapRuntime<C, E>: Send + Sync + Sized {
    fn build(config: C) -> impl Future<Output = io::Result<Self>> + Send;
    fn build_https(config: C) -> impl Future<Output = io::Result<Self>> + Send;
    fn build_hosted_https(config: C) -> impl Future<Output = io::Result<Self>> + Send;
    fn build_with_owners(
        config: C,
        listener: Option<TcpListener>,
        api_base: String,
        owners: BootstrapOwners<E>,
    ) -> impl Future<Output = io::Result<Self>> + Send;
    fn endpoint_id(&self) -> iroh::EndpointId;
    fn cancellation_token(&self) -> CancellationToken;
    fn join_owner(&self) -> &OwnerProvisioner;
    fn node_public_ready(&self) -> impl Future<Output = io::Result<bool>> + Send;
    fn node_join_public_ready(&self) -> impl Future<Output = io::Result<bool>> + Send;
    fn start_background_tasks(&mut self) -> impl Future<Output = io::Result<()>> + Send;
    fn verify_local_bootstrap(
        self,
        tls: Option<&LocalTls>,
    ) -> impl Future<Output = io::Result<()>> + Send;
    fn shutdown(self) -> impl Future<Output = io::Result<()>> + Send;
}

pub trait BootstrapProtocol<E>: Send + Sync + Sized {
    fn open_node_bootstrap(
        endpoint: &E,
        config: &AppConfig,
        management_directory: &Path,
        binding: NodeSignerBinding,
    ) -> impl Future<Output = io::Result<(Self, OwnerProvisioner)>> + Send;
    fn join_from_trusted_peer(
        &self,
        owner: &OwnerProvisioner,
        name: &str,
        peer: &str,
    ) -> impl Future<Output = io::Result<management_service::hosted_join::NodeJoinApproval>> + Send;
    fn into_runtime_owners(self, endpoint: Arc<E>) -> BootstrapOwners<E>;
}
