use db::Database;
pub use device_service::security_policy::SecurityContext;
use idp_server::OwnerProvisioner;
use idp_service::oauth2::NodeSignerBinding;
use std::{
    io,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};
use storage_server::DataProtocolHandler;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct SecurityProtocol {
    owner: device_service::security_protocol::SecurityProtocol,
}

impl Deref for SecurityProtocol {
    type Target = device_service::security_protocol::SecurityProtocol;
    fn deref(&self) -> &Self::Target {
        &self.owner
    }
}

impl SecurityProtocol {
    pub fn new(
        server: iroh_chain::Server,
        idp: Arc<Database>,
        management: Arc<Database>,
        policy: Arc<Database>,
        installation: String,
        binding: Option<NodeSignerBinding>,
        staging_directory: PathBuf,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            owner: device_service::security_protocol::SecurityProtocol::new(
                server,
                idp,
                management,
                policy,
                installation,
                binding,
                staging_directory,
                cancellation,
            ),
        }
    }

    pub fn attach(&self, handler: DataProtocolHandler) -> DataProtocolHandler {
        let owner = self.owner.clone();
        handler.with_additional_stream(move |kind, connection, send, recv| {
            let owner = owner.clone();
            async move { owner.accept_data_stream(kind, connection, send, recv).await }
        })
    }

    pub fn into_runtime_owners(
        self,
        endpoint: Arc<crate::UnifiedEndpoint>,
    ) -> crate::runtime::RuntimeOwners {
        let (idp, devices) = self.owner.into_databases();
        crate::runtime::RuntimeOwners {
            endpoint,
            idp,
            devices,
        }
    }

    pub async fn open_node_bootstrap(
        endpoint: &crate::UnifiedEndpoint,
        config: &idp_server::AppConfig,
        management_directory: &Path,
        binding: NodeSignerBinding,
    ) -> io::Result<(Self, OwnerProvisioner)> {
        device_service::node_join_owner::check_bootstrap_endpoint(
            endpoint.endpoint_id(),
            &binding,
        )?;
        let owner = OwnerProvisioner::open(config).await?;
        let management = device_server::open_local_device_database(management_directory).await?;
        let protocol = Self::new(
            endpoint.server().clone(),
            owner.node_join_database()?,
            management,
            owner.node_join_database()?,
            config.oauth2.issuer.clone(),
            Some(binding),
            management_directory.join("node-bootstrap-staging"),
            CancellationToken::new(),
        );
        Ok((protocol, owner))
    }
}
