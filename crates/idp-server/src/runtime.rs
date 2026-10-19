use std::{io, sync::Arc, time::Duration};

use axum::Router;
use db::Database;
use idp_model::model::Id;
use idp_service::{
    oauth2::OAuth2Service,
    replica::{
        DbApplicationRepo, DbClientRepo, DbKeyRepo, DbOAuth2AuthorizationCodeRepo,
        DbOAuth2RefreshTokenRepo, DbOAuth2UserConsentRepo, DbUserRepo,
    },
    repo::{KeyService, PrivateKeyKeyringRepo},
};
use iroh::protocol::{ProtocolHandler, Router as IrohRouter};
use iroh_chain::Server;
use management_service::{
    DeviceIdentity, HostedControlPlane, LocalPermissions, PairingAcceptanceController,
    TimedPairingAcceptanceController, replica::DbDeviceRepo,
};

use crate::{
    AppConfig, BootstrapProtocolHandler, ManagementPermissionClient, RouterState,
    bootstrap::BOOTSTRAP_ALPN, router::openapi_router,
};

pub struct IdpRuntime {
    router: Router,
    bootstrap_protocol: BootstrapProtocolHandler,
    join_state: RouterState,
}

impl IdpRuntime {
    pub async fn authorize_management_bearer(
        &self,
        bearer: &str,
    ) -> Result<idp_model::contract::IntrospectionResponse, String> {
        crate::management_owner::authorize_management_bearer(&self.join_state, bearer).await
    }

    pub async fn active_user(&self, user_id: Id) -> Result<(), String> {
        crate::management_owner::active_management_user(&self.join_state, user_id).await
    }

    pub fn pairing_acceptance(&self) -> Arc<management_service::PairingAcceptanceControllerSlot> {
        Arc::clone(&self.join_state.pairing_acceptance)
    }

    pub async fn authorize_storage_bearer(
        &self,
        bearer: &str,
    ) -> Result<idp_model::contract::IntrospectionResponse, idp_model::contract::ErrorResponse>
    {
        crate::storage_owner::authorize_storage_bearer(&self.join_state, bearer).await
    }

    pub async fn authorize_node_join_approver(
        &self,
        bearer: &str,
    ) -> io::Result<idp_model::model::Id> {
        self.join_state.authorize_node_join_approver(bearer).await
    }

    pub async fn node_join_accepting(
        &self,
        bearer: &str,
        accepting: Option<bool>,
    ) -> io::Result<bool> {
        self.authorize_node_join_approver(bearer).await?;
        if let Some(accepting) = accepting {
            self.join_state
                .pairing_acceptance
                .set_pairing_accepting(accepting)
                .map_err(io::Error::other)?;
        }
        self.join_state
            .pairing_acceptance
            .pairing_accepting()
            .map_err(io::Error::other)
    }

    pub async fn approve_node_join(
        &self,
        offer: &iroh_chain::PairingOffer,
        bearer: &str,
    ) -> io::Result<crate::NodeJoinApproval> {
        self.join_state.approve_node_join(offer, bearer).await
    }

    pub async fn enroll_node_join_offer(
        &self,
        offer: &iroh_chain::PairingOffer,
        bearer: &str,
    ) -> io::Result<crate::NodeJoinApproval> {
        self.join_state.enroll_node_join_offer(offer, bearer).await
    }

    pub async fn verify_node_join_enrollment(
        &self,
        approval: &crate::NodeJoinApproval,
    ) -> io::Result<()> {
        self.join_state.verify_node_join_enrollment(approval).await
    }

    pub async fn reply_node_join(
        &self,
        offer: &iroh_chain::PairingOffer,
        bearer: &str,
        approval: &crate::NodeJoinApproval,
    ) -> io::Result<()> {
        self.join_state
            .reply_node_join(offer, bearer, approval)
            .await
    }

    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn bootstrap_protocol(&self) -> BootstrapProtocolHandler {
        self.bootstrap_protocol.clone()
    }

    pub fn protocol_router<D>(&self, server: &Server, data_protocol: D) -> IrohRouter
    where
        D: ProtocolHandler,
    {
        server.router_with_protocol(data_protocol, BOOTSTRAP_ALPN, self.bootstrap_protocol())
    }
}

/// Composes serving from an already initialized IdP database and optional host-owned Management capability.
pub async fn build_runtime(
    config: &AppConfig,
    engine: Arc<Database>,
    devices: Option<Arc<DbDeviceRepo>>,
    device_identity: Arc<DeviceIdentity>,
    server: Server,
    local_permissions: Option<LocalPermissions>,
) -> io::Result<IdpRuntime> {
    let remote_permissions = if local_permissions.is_some() {
        None
    } else if let Some(remote) = config.management_permissions.as_ref() {
        if config.management_data_dir.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote Management evaluation and a local Management data directory are mutually exclusive",
            ));
        }
        Some(ManagementPermissionClient::new(remote)?)
    } else {
        None
    };
    if device_identity.endpoint_id() != server.endpoint().id() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "device identity does not match the injected Iroh server",
        ));
    }

    if config
        .oauth2
        .local_signer
        .as_ref()
        .is_some_and(|binding| binding.endpoint_id != device_identity.endpoint_id().to_string())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local signer endpoint does not match the injected Node endpoint",
        ));
    }

    let key_service = Arc::new(KeyService::new(
        DbKeyRepo::new(Arc::clone(&engine)),
        PrivateKeyKeyringRepo::new(&config.oauth2.issuer).map_err(io::Error::other)?,
        config.key_namespace.clone(),
    ));
    if let Some(binding) = config.oauth2.local_signer.as_ref() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_secs()
            .try_into()
            .map_err(io::Error::other)?;
        key_service
            .load_node_signer(binding, &config.oauth2.issuer, now)
            .await
            .map_err(io::Error::other)?;
    }
    let oauth2_service = Arc::new(OAuth2Service::new(
        DbApplicationRepo::new(Arc::clone(&engine)),
        DbClientRepo::new(Arc::clone(&engine), Arc::clone(&key_service)),
        DbOAuth2AuthorizationCodeRepo::new(Arc::clone(&engine)),
        DbOAuth2RefreshTokenRepo::new(Arc::clone(&engine)),
        DbUserRepo::new(Arc::clone(&engine), config.password.clone()),
        DbOAuth2UserConsentRepo::new(Arc::clone(&engine)),
        key_service,
        config.oauth2.clone(),
    ));

    let control_plane = config
        .control_plane_uri
        .as_deref()
        .map(HostedControlPlane::new)
        .transpose()
        .map_err(io::Error::other)?
        .map(Arc::new);
    let service_audience = config
        .service_audience
        .as_deref()
        .unwrap_or(&config.api_public_uri);
    if service_audience.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "service_audience must not be empty",
        ));
    }

    let router_state = RouterState::new(
        &config.ui_public_uri,
        &config.api_public_uri,
        Arc::clone(&engine),
        Arc::clone(&oauth2_service),
        devices,
        device_identity,
    )
    .with_service_audience(service_audience);
    let router_state = match control_plane {
        Some(control_plane) => router_state.with_hosted_control_plane(control_plane),
        None => router_state,
    };

    let router_state = match local_permissions {
        Some(local) => router_state.with_local_permissions(local),
        None => router_state,
    };
    let router_state = match remote_permissions {
        Some(remote) => router_state.with_management_permissions(remote),
        None => router_state,
    };

    router_state
        .pairing_acceptance
        .bind(Arc::new(TimedPairingAcceptanceController::new(
            server,
            Duration::from_secs(config.pairing.accepting_timeout_seconds),
        )))
        .map_err(io::Error::other)?;

    let bootstrap_protocol = BootstrapProtocolHandler;
    let router = openapi_router(router_state.clone(), config.server.prefix())
        .split_for_parts()
        .0;

    Ok(IdpRuntime {
        router,
        bootstrap_protocol,
        join_state: router_state,
    })
}

#[cfg(test)]
mod tests {
    use crate::node_join_tests::approval_fixture;

    #[tokio::test]
    async fn active_user_rechecks_current_identity_without_management_transport_state() {
        use idp_service::repo::UserRepo;
        let fixture = approval_fixture().await;
        let unopened = fixture.root.join("must-not-open-management");
        let config = crate::AppConfig {
            management_data_dir: Some(unopened.to_string_lossy().into_owned()),
            oauth2: idp_service::oauth2::OAuth2Config {
                issuer: fixture.issuer.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        let runtime = super::build_runtime(
            &config,
            fixture.state.engine.clone(),
            None,
            fixture.state.device_identity.clone(),
            fixture.server.clone(),
            None,
        )
        .await
        .expect("independent IdP with no Management transport state");
        assert!(
            !unopened.exists(),
            "IdP runtime must not open or migrate Management"
        );
        runtime
            .active_user(fixture.owner)
            .await
            .expect("current local User");
        assert!(
            runtime
                .active_user(idp_model::model::Id::nil())
                .await
                .is_err()
        );
        assert!(
            runtime
                .active_user(idp_model::model::Id::now_v7())
                .await
                .is_err()
        );
        assert!(
            runtime.join_state.management_devices().is_err(),
            "missing Management capability cannot fall back to IdP"
        );
        fixture
            .state
            .oauth2_service
            .user_repo
            .delete_user_by_id(fixture.owner)
            .await
            .expect("learn User deletion");
        assert!(runtime.active_user(fixture.owner).await.is_err());
        drop(runtime);
        fixture.server.endpoint().close().await;
        let root = fixture.root.clone();
        drop(fixture);
        std::fs::remove_dir_all(root).expect("remove isolated runtime fixture");
    }
}
