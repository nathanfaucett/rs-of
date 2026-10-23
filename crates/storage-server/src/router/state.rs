#[cfg(feature = "network")]
use crate::LocalReplicationAuthorizer;
use crate::{DeviceClient, IdpClient};

#[derive(Clone)]
pub struct RouterState {
    pub api_base_uri: String,
    pub idp_client: Option<IdpClient>,
    pub device_client: Option<DeviceClient>,
    #[cfg(feature = "network")]
    pub local_replication_authorizer: Option<LocalReplicationAuthorizer>,
}

impl RouterState {
    pub fn new(api_base_uri: impl Into<String>) -> Self {
        Self {
            api_base_uri: api_base_uri.into(),
            idp_client: None,
            device_client: None,
            #[cfg(feature = "network")]
            local_replication_authorizer: None,
        }
    }

    pub fn with_idp_client(mut self, idp_client: IdpClient) -> Self {
        self.idp_client = Some(idp_client);
        self
    }

    #[cfg(feature = "network")]
    pub fn with_local_replication_authorizer(
        mut self,
        authorizer: LocalReplicationAuthorizer,
    ) -> Self {
        self.local_replication_authorizer = Some(authorizer);
        self
    }

    pub fn with_device_client(mut self, device_client: DeviceClient) -> Self {
        self.device_client = Some(device_client);
        self
    }
}
