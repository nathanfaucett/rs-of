use std::sync::Arc;

use crate::ManagementPermissionClient;
use idp_service::{
    oauth2::OAuth2Service,
    replica::{
        DbApplicationRepo, DbClientRepo, DbKeyRepo, DbOAuth2AuthorizationCodeRepo,
        DbOAuth2RefreshTokenRepo, DbOAuth2UserConsentRepo, DbReplicaSignerRepo, DbUserRepo,
    },
    repo::PrivateKeyKeyringRepo,
};
use management_service::{
    DeviceIdentity, HostedControlPlane, LocalPermissions, PairingAcceptanceControllerSlot,
    replica::DbDeviceRepo,
};

pub(super) type NativeOAuth2Service = OAuth2Service<
    DbApplicationRepo,
    DbClientRepo,
    DbOAuth2AuthorizationCodeRepo,
    DbOAuth2RefreshTokenRepo,
    DbUserRepo,
    DbOAuth2UserConsentRepo,
    DbKeyRepo,
    PrivateKeyKeyringRepo,
>;

pub type NativeDeviceRepo = DbDeviceRepo;
pub type NativeReplicaSignerRepo = DbReplicaSignerRepo;

#[derive(Clone)]
pub struct RouterState {
    pub ui_base_uri: String,
    pub api_base_uri: String,
    pub service_audience: String,
    pub engine: Arc<db::Database>,
    pub oauth2_service: Arc<NativeOAuth2Service>,
    pub devices: Arc<NativeDeviceRepo>,
    pub replica_signers: Arc<NativeReplicaSignerRepo>,
    pub device_identity: Arc<DeviceIdentity>,
    pub pairing_acceptance: Arc<PairingAcceptanceControllerSlot>,
    pub hosted_control_plane: Option<Arc<HostedControlPlane>>,
    pub local_permissions: Option<Arc<LocalPermissions>>,
    pub management_permissions: Option<Arc<ManagementPermissionClient>>,
    #[cfg(test)]
    pub(crate) before_node_join_activation:
        Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>,
}

impl RouterState {
    pub fn new(
        ui_base_uri: impl Into<String>,
        api_base_uri: impl Into<String>,
        engine: Arc<db::Database>,
        oauth2_service: Arc<NativeOAuth2Service>,
        devices: Arc<NativeDeviceRepo>,
        device_identity: Arc<DeviceIdentity>,
    ) -> Self {
        let api_base_uri = api_base_uri.into();
        let replica_signers = Arc::new(DbReplicaSignerRepo::new(Arc::clone(&engine)));
        Self {
            ui_base_uri: ui_base_uri.into(),
            service_audience: api_base_uri.clone(),
            api_base_uri,
            engine,
            oauth2_service,
            replica_signers,
            devices,
            device_identity,
            pairing_acceptance: Arc::new(PairingAcceptanceControllerSlot::new()),
            hosted_control_plane: None,
            local_permissions: None,
            management_permissions: None,
            #[cfg(test)]
            before_node_join_activation: None,
        }
    }

    pub fn with_local_permissions(mut self, client: LocalPermissions) -> Self {
        self.local_permissions = Some(Arc::new(client));
        self
    }

    pub fn with_management_permissions(mut self, client: ManagementPermissionClient) -> Self {
        self.management_permissions = Some(Arc::new(client));
        self
    }

    pub fn with_service_audience(mut self, audience: impl Into<String>) -> Self {
        self.service_audience = audience.into();
        self
    }

    pub fn with_hosted_control_plane(mut self, control_plane: Arc<HostedControlPlane>) -> Self {
        self.hosted_control_plane = Some(control_plane);
        self
    }
}
