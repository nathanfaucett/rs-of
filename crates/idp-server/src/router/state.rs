use std::sync::Arc;

use device_service::DeviceIdentity;
use device_service::HostedControlPlane;
use device_service::PairingAcceptanceControllerSlot;
use device_service::replica::DbDeviceRepo;
use idp_service::{
    oauth2::OAuth2Service,
    replica::{
        DbApplicationRepo, DbClientRepo, DbKeyRepo, DbOAuth2AuthorizationCodeRepo,
        DbOAuth2RefreshTokenRepo, DbOAuth2UserConsentRepo, DbReplicaSignerRepo, DbUserRepo,
    },
    repo::PrivateKeyKeyringRepo,
};
use management_service::LocalPermissions;

use crate::ManagementPermissionClient;

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
    pub engine: db::Client,
    pub oauth2_service: Arc<NativeOAuth2Service>,
    pub devices: Option<Arc<NativeDeviceRepo>>,
    pub replica_signers: Arc<NativeReplicaSignerRepo>,
    pub(crate) device_identity: Option<Arc<DeviceIdentity>>,
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
        engine: db::Client,
        oauth2_service: Arc<NativeOAuth2Service>,
        devices: Option<Arc<NativeDeviceRepo>>,
        device_identity: Arc<DeviceIdentity>,
    ) -> Self {
        let mut state = Self::new_oauth(ui_base_uri, api_base_uri, engine, oauth2_service);
        state.devices = devices;
        state.device_identity = Some(device_identity);
        state
    }

    pub fn new_oauth(
        ui_base_uri: impl Into<String>,
        api_base_uri: impl Into<String>,
        engine: db::Client,
        oauth2_service: Arc<NativeOAuth2Service>,
    ) -> Self {
        let api_base_uri = api_base_uri.into();
        let replica_signers = Arc::new(DbReplicaSignerRepo::new(engine.clone()));
        Self {
            ui_base_uri: ui_base_uri.into(),
            service_audience: api_base_uri.clone(),
            api_base_uri,
            engine,
            oauth2_service,
            replica_signers,
            devices: None,
            device_identity: None,
            pairing_acceptance: Arc::new(PairingAcceptanceControllerSlot::new()),
            hosted_control_plane: None,
            local_permissions: None,
            management_permissions: None,
            #[cfg(test)]
            before_node_join_activation: None,
        }
    }

    pub(crate) fn transport_identity(&self) -> std::io::Result<&Arc<DeviceIdentity>> {
        self.device_identity.as_ref().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Management transport identity is unavailable",
            )
        })
    }

    pub(crate) fn management_devices(&self) -> std::io::Result<&Arc<NativeDeviceRepo>> {
        self.devices.as_ref().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Management Device owner is unavailable",
            )
        })
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
