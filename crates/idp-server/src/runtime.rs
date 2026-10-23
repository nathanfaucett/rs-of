use std::{io, sync::Arc, time::Duration};

use axum::Router;
use db::Database;
use device_service::DeviceIdentity;
use device_service::HostedControlPlane;
use device_service::PairingAcceptanceController;
use device_service::TimedPairingAcceptanceController;
use device_service::replica::DbDeviceRepo;
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
use management_service::LocalPermissions;

use crate::{
    AppConfig, BootstrapProtocolHandler, ManagementPermissionClient, RouterState,
    bootstrap::BOOTSTRAP_ALPN, router::openapi_router,
};

pub struct IdpRuntime {
    pub(crate) router: Router,
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

    pub fn pairing_acceptance(&self) -> Arc<device_service::PairingAcceptanceControllerSlot> {
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
    ) -> io::Result<device_service::hosted_join::NodeJoinApproval> {
        self.join_state.approve_node_join(offer, bearer).await
    }

    pub async fn enroll_node_join_offer(
        &self,
        offer: &iroh_chain::PairingOffer,
        bearer: &str,
    ) -> io::Result<device_service::hosted_join::NodeJoinApproval> {
        self.join_state.enroll_node_join_offer(offer, bearer).await
    }

    pub async fn verify_node_join_enrollment(
        &self,
        approval: &device_service::hosted_join::NodeJoinApproval,
    ) -> io::Result<()> {
        self.join_state.verify_node_join_enrollment(approval).await
    }

    pub async fn reply_node_join(
        &self,
        offer: &iroh_chain::PairingOffer,
        bearer: &str,
        approval: &device_service::hosted_join::NodeJoinApproval,
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
    engine: db::Client,
    devices: Option<Arc<DbDeviceRepo>>,
    device_identity: Arc<DeviceIdentity>,
    server: Server,
    local_permissions: Option<LocalPermissions>,
) -> io::Result<IdpRuntime> {
    if config.identity_database.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "injected identity owner and remote SQL are mutually exclusive",
        ));
    }
    if device_identity.endpoint_id() != server.endpoint().id() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "device identity does not match the injected Iroh server",
        ));
    }
    let state = build_state(
        config,
        engine,
        devices,
        Some(device_identity),
        local_permissions,
    )
    .await?;
    state
        .pairing_acceptance
        .bind(Arc::new(TimedPairingAcceptanceController::new(
            server,
            Duration::from_secs(config.pairing.accepting_timeout_seconds),
        )))
        .map_err(io::Error::other)?;
    Ok(runtime_from_state(config, state))
}

pub async fn identity_client(config: &AppConfig) -> io::Result<db::Client> {
    if let Some(remote) = &config.identity_database {
        if std::path::Path::new(&config.data_dir)
            .join("idp.redb")
            .try_exists()?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote identity consumer cannot also own idp.redb",
            ));
        }
        if config.management_data_dir.is_some() || config.management_permissions.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote identity SQL cannot be combined with a separate Management policy source",
            ));
        }
        let client = remote.connect().await?;
        idp_model::replica::validate_identity(&client)
            .await
            .map_err(io::Error::other)?;
        return Ok(client);
    }
    std::fs::create_dir_all(&config.data_dir)?;
    let database = Database::open(std::path::Path::new(&config.data_dir).join("idp.redb"))
        .map_err(io::Error::other)?;
    idp_model::replica::up(&database)
        .await
        .map_err(io::Error::other)?;
    Ok(database.client())
}

/// Builds an IdP HTTPS owner without acquiring transport identity or Management state.
pub async fn build_oauth_runtime(config: &AppConfig, engine: db::Client) -> io::Result<IdpRuntime> {
    let permissions = config
        .identity_database
        .as_ref()
        .map(|_| LocalPermissions::new(engine.clone()));
    let state = build_state(config, engine, None, None, permissions).await?;
    Ok(runtime_from_state(config, state))
}

pub(crate) async fn build_state(
    config: &AppConfig,
    engine: db::Client,
    devices: Option<Arc<DbDeviceRepo>>,
    device_identity: Option<Arc<DeviceIdentity>>,
    local_permissions: Option<LocalPermissions>,
) -> io::Result<RouterState> {
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
    let key_service = Arc::new(KeyService::new(
        DbKeyRepo::new(engine.clone()),
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
        DbApplicationRepo::new(engine.clone()),
        DbClientRepo::new(engine.clone(), Arc::clone(&key_service)),
        DbOAuth2AuthorizationCodeRepo::new(engine.clone()),
        DbOAuth2RefreshTokenRepo::new(engine.clone()),
        DbUserRepo::new(engine.clone(), config.password.clone()),
        DbOAuth2UserConsentRepo::new(engine.clone()),
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

    let mut router_state = RouterState::new_oauth(
        &config.ui_public_uri,
        &config.api_public_uri,
        engine,
        Arc::clone(&oauth2_service),
    )
    .with_service_audience(service_audience);
    router_state.devices = devices;
    router_state.device_identity = device_identity;
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

    Ok(router_state)
}

pub(crate) fn runtime_from_state(config: &AppConfig, router_state: RouterState) -> IdpRuntime {
    let bootstrap_protocol = BootstrapProtocolHandler;
    let router = openapi_router(router_state.clone(), config.server.prefix())
        .split_for_parts()
        .0;

    IdpRuntime {
        router,
        bootstrap_protocol,
        join_state: router_state,
    }
}

impl device_service::hosted_join::NodeJoinIdentity for IdpRuntime {
    fn accepting<'a>(
        &'a self,
        bearer: &'a str,
        accepting: Option<bool>,
    ) -> device_service::hosted_join::JoinFuture<'a, bool> {
        Box::pin(self.node_join_accepting(bearer, accepting))
    }
    fn authorize<'a>(&'a self, bearer: &'a str) -> device_service::hosted_join::JoinFuture<'a, ()> {
        Box::pin(async move { self.authorize_node_join_approver(bearer).await.map(|_| ()) })
    }
    fn enroll<'a>(
        &'a self,
        offer: &'a iroh_chain::PairingOffer,
        bearer: &'a str,
    ) -> device_service::hosted_join::JoinFuture<'a, device_service::hosted_join::NodeJoinApproval>
    {
        Box::pin(self.enroll_node_join_offer(offer, bearer))
    }
    fn verify<'a>(
        &'a self,
        approval: &'a device_service::hosted_join::NodeJoinApproval,
    ) -> device_service::hosted_join::JoinFuture<'a, ()> {
        Box::pin(self.verify_node_join_enrollment(approval))
    }
    fn reply<'a>(
        &'a self,
        offer: &'a iroh_chain::PairingOffer,
        bearer: &'a str,
        approval: &'a device_service::hosted_join::NodeJoinApproval,
    ) -> device_service::hosted_join::JoinFuture<'a, ()> {
        Box::pin(self.reply_node_join(offer, bearer, approval))
    }
}

#[cfg(test)]
mod tests {
    use device_service::PairingAcceptanceController;

    use crate::node_join_tests::approval_fixture;

    #[tokio::test]
    async fn remote_identity_startup_never_falls_back_to_a_private_database() {
        let root = std::env::temp_dir().join(format!(
            "remote-identity-startup-{}",
            idp_model::model::Id::now_v7()
        ));
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("reserve unavailable SQL endpoint");
        let address = listener.local_addr().expect("SQL endpoint");
        drop(listener);
        let mut config = crate::AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            identity_database: Some(db::RemoteDatabaseConfig {
                endpoint: format!("http://{address}"),
                ca_certificate: None,
                client_certificate: None,
                client_key: None,
                deadline_ms: 100,
            }),
            ..Default::default()
        };
        assert!(super::identity_client(&config).await.is_err());
        assert!(!root.exists());
        config
            .identity_database
            .as_mut()
            .expect("remote config")
            .endpoint = "http://sql.example:50051".into();
        assert!(super::identity_client(&config).await.is_err());
        assert!(!root.exists());
    }

    async fn assert_normal_oauth(runtime: &super::IdpRuntime) {
        use axum::{
            body::{Body, to_bytes},
            http::{Request, StatusCode},
        };
        use base64::{Engine, engine::general_purpose::STANDARD};
        use tower::ServiceExt;

        let response = runtime.router().oneshot(Request::builder()
            .method("POST").uri("/oauth2/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Authorization", format!("Basic {}", STANDARD.encode("human-client:human-secret")))
            .body(Body::from("grant_type=password&client_id=human-client&username=admin&password=valid-password"))
            .expect("normal token request")).await.expect("normal token response");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "retained signer must issue normal tokens"
        );
        let token: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), 65536)
                .await
                .expect("bounded token response"),
        )
        .expect("token JSON");
        assert!(
            token["access_token"]
                .as_str()
                .is_some_and(|token| !token.is_empty())
        );
        let response = runtime
            .router()
            .oneshot(
                Request::builder()
                    .uri("/userinfo")
                    .header("Authorization", "Bearer forged-bearer")
                    .body(Body::empty())
                    .expect("forged request"),
            )
            .await
            .expect("normal protected response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(
            runtime.join_state.management_devices().is_err(),
            "OAuth serving does not acquire Management authority"
        );
    }

    #[tokio::test]
    async fn independent_oauth_signer_does_not_claim_the_transport_endpoint() {
        use std::sync::Arc;

        let fixture = approval_fixture().await;
        let secret = iroh::SecretKey::generate();
        let transport = iroh_chain::Server::bind_with_secret_key(
            iroh::endpoint::presets::Minimal,
            secret.clone(),
            iroh_chain::EndpointIdStore::default(),
        )
        .await
        .expect("independent transport");
        let identity = Arc::new(
            device_service::DeviceIdentity::new(transport.endpoint().clone(), secret)
                .expect("transport possession"),
        );
        let config = crate::AppConfig {
            oauth2: fixture.state.oauth2_service.oauth_config.clone(),
            ..Default::default()
        };
        assert_ne!(
            config
                .oauth2
                .local_signer
                .as_ref()
                .expect("signer")
                .endpoint_id,
            transport.endpoint().id().to_string()
        );
        let mismatch = super::build_runtime(
            &config,
            fixture.state.engine.clone(),
            None,
            fixture
                .state
                .transport_identity()
                .expect("fixture transport")
                .clone(),
            transport.clone(),
            None,
        )
        .await
        .err()
        .expect("transport identity still requires possession of the injected endpoint key");
        assert_eq!(mismatch.kind(), std::io::ErrorKind::InvalidInput);
        let runtime = super::build_runtime(
            &config,
            fixture.state.engine.clone(),
            None,
            identity,
            transport.clone(),
            None,
        )
        .await
        .expect("OAuth signer is independent of transport");
        let oauth_only = super::build_oauth_runtime(&config, fixture.state.engine.clone())
            .await
            .expect("OAuth runtime needs no transport or Management store");
        assert!(oauth_only.join_state.transport_identity().is_err());
        for runtime in [&runtime, &oauth_only] {
            assert_normal_oauth(runtime).await;
        }
        drop(oauth_only);
        drop(runtime);
        transport.endpoint().close().await;
        fixture.server.endpoint().close().await;
        let root = fixture.root.clone();
        drop(fixture);
        let engine =
            Arc::new(db::Database::open(root.join("idp.redb")).expect("stopped owner reopen"));
        let reopened = super::build_oauth_runtime(&config, engine.client())
            .await
            .expect("offline OAuth-only reopen");
        assert_normal_oauth(&reopened).await;
        assert!(reopened.join_state.transport_identity().is_err());
        assert!(reopened.pairing_acceptance().pairing_accepting().is_err());
        drop(reopened);
        let mut invalid = config;
        invalid.oauth2.local_signer.as_mut().expect("signer").key_id =
            idp_model::model::Id::now_v7();
        let engine = Arc::new(db::Database::open(root.join("idp.redb")).expect("existing owner"));
        assert!(
            super::build_oauth_runtime(&invalid, engine.client())
                .await
                .is_err(),
            "independence must not bypass approved signer/private-key validation"
        );
        std::fs::remove_dir_all(root).expect("isolated cleanup");
    }

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
            fixture
                .state
                .transport_identity()
                .expect("fixture transport")
                .clone(),
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
