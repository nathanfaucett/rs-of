use std::{io, sync::Arc, time::Duration};

use axum::Router;
use device_service::{
    DeviceIdentity, HostedControlPlane, PairingAcceptanceController,
    TimedPairingAcceptanceController, replica::DbDeviceRepo,
};
use idp_service::{
    oauth2::service::OAuth2Service,
    replica::{
        DbApplicationRepo, DbClientRepo, DbKeyRepo, DbOAuth2AuthorizationCodeRepo,
        DbOAuth2RefreshTokenRepo, DbOAuth2UserConsentRepo, DbUserRepo,
    },
    repo::{KeyService, PrivateKeyKeyringRepo},
};
use iroh::protocol::{ProtocolHandler, Router as IrohRouter};
use iroh_chain::Server;
use management_service::LocalPermissions;
use of_model::id::Id;
use ofdb_sql::Database;

use crate::bootstrap::BOOTSTRAP_ALPN;
use crate::bootstrap::handler::BootstrapProtocolHandler;
use crate::config::app_config::AppConfig;
use crate::management_permission_client::ManagementPermissionClient;
use crate::router::openapi_router::openapi_router;
use crate::router::state::RouterState;

pub struct IdpRuntime {
    pub(crate) router: Router,
    bootstrap_protocol: BootstrapProtocolHandler,
    join_state: RouterState,
}

impl IdpRuntime {
    pub async fn authorize_management_bearer(
        &self,
        bearer: &str,
    ) -> Result<of_model::idp::contract::introspection_response::IntrospectionResponse, String>
    {
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
    ) -> Result<
        of_model::idp::contract::introspection_response::IntrospectionResponse,
        of_model::idp::contract::error_response::ErrorResponse,
    > {
        crate::storage_owner::authorize_storage_bearer(&self.join_state, bearer).await
    }

    pub async fn authorize_node_join_approver(&self, bearer: &str) -> io::Result<of_model::id::Id> {
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
    ) -> io::Result<of_model::node_join::NodeJoinApproval> {
        self.join_state.approve_node_join(offer, bearer).await
    }

    pub async fn enroll_node_join_offer(
        &self,
        offer: &iroh_chain::PairingOffer,
        bearer: &str,
    ) -> io::Result<of_model::node_join::NodeJoinApproval> {
        self.join_state.enroll_node_join_offer(offer, bearer).await
    }

    pub async fn verify_node_join_enrollment(
        &self,
        approval: &of_model::node_join::NodeJoinApproval,
    ) -> io::Result<()> {
        self.join_state.verify_node_join_enrollment(approval).await
    }

    pub async fn reply_node_join(
        &self,
        offer: &iroh_chain::PairingOffer,
        bearer: &str,
        approval: &of_model::node_join::NodeJoinApproval,
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

/// Builds the local IdP without setup, admission, or a transport endpoint.
pub async fn build_local_router(
    data_dir: &std::path::Path,
    base: &str,
    origin: &str,
    db: Option<Arc<ofdb_sql::Database>>,
    users: &[(String, String)],
) -> io::Result<Router> {
    std::fs::create_dir_all(data_dir)?;
    let directory = std::fs::canonicalize(data_dir)?;
    let data_dir = directory.as_path();
    let database = db::local::local_database(data_dir, db).await?;
    let engine = database.client();
    let storage = database.storage();

    let mut config = AppConfig {
        data_dir: data_dir.to_string_lossy().into_owned(),
        ui_public_uri: origin.trim_end_matches('/').into(),
        api_public_uri: base.into(),
        key_namespace: "of:local".into(),
        password: idp_service::password_config::PasswordConfig {
            memory_mib: 1,
            iterations: 1,
            parallelism: 1,
            ..Default::default()
        },
        ..AppConfig::default()
    };
    config.oauth2.issuer = base.into();
    config.oauth2.local_signer = Some(
        crate::local_seed::signer(
            engine.clone(),
            &storage,
            &config.key_namespace,
            &config.oauth2.issuer,
        )
        .await?,
    );
    let permissions = LocalPermissions::new(engine.clone());
    let keys = PrivateKeyKeyringRepo::new_local(&storage).map_err(io::Error::other)?;
    let state = build_state_with_keys(&config, engine, None, None, Some(permissions), keys).await?;
    crate::local_seed::seed(&state, origin).await?;
    for (username, password) in users {
        let user = of_model::id::Id::new_v5(
            &of_model::id::Id::NAMESPACE_URL,
            format!("of:local:user:{username}").as_bytes(),
        );
        crate::local_seed::seed_user(&state, user, username, password).await?;
    }
    Ok(openapi_router(state, "/idp").into())
}

/// Composes serving from an already initialized IdP database and optional host-owned Management capability.
pub async fn build_runtime(
    config: &AppConfig,
    engine: ofdb_sql::Client,
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

pub async fn identity_client(config: &AppConfig) -> io::Result<ofdb_sql::Client> {
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
                "remote Management evaluation and a local Management data directory are mutually exclusive",
            ));
        }
        let client = remote.connect().await?;
        db::ofdb::validators::validate_identity(&client)
            .await
            .map_err(io::Error::other)?;
        return Ok(client);
    }
    std::fs::create_dir_all(&config.data_dir)?;
    let database = db::local::local_database(&std::path::Path::new(&config.data_dir), None).await?;
    Ok(database.client())
}

/// Builds an IdP HTTPS owner without acquiring transport identity or Management state.
pub async fn build_oauth_runtime(
    config: &AppConfig,
    engine: ofdb_sql::Client,
) -> io::Result<IdpRuntime> {
    let permissions = config
        .identity_database
        .as_ref()
        .map(|_| LocalPermissions::new(engine.clone()));
    let state = build_state(config, engine, None, None, permissions).await?;
    Ok(runtime_from_state(config, state))
}

pub(crate) async fn build_state(
    config: &AppConfig,
    engine: ofdb_sql::Client,
    devices: Option<Arc<DbDeviceRepo>>,
    device_identity: Option<Arc<DeviceIdentity>>,
    local_permissions: Option<LocalPermissions>,
) -> io::Result<RouterState> {
    let keys = PrivateKeyKeyringRepo::new(&config.oauth2.issuer).map_err(io::Error::other)?;
    build_state_with_keys(
        config,
        engine,
        devices,
        device_identity,
        local_permissions,
        keys,
    )
    .await
}

async fn build_state_with_keys(
    config: &AppConfig,
    engine: ofdb_sql::Client,
    devices: Option<Arc<DbDeviceRepo>>,
    device_identity: Option<Arc<DeviceIdentity>>,
    local_permissions: Option<LocalPermissions>,
    keys: PrivateKeyKeyringRepo,
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
        keys,
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
    ) -> device_service::hosted_join::JoinFuture<'a, of_model::node_join::NodeJoinApproval> {
        Box::pin(self.enroll_node_join_offer(offer, bearer))
    }
    fn verify<'a>(
        &'a self,
        approval: &'a of_model::node_join::NodeJoinApproval,
    ) -> device_service::hosted_join::JoinFuture<'a, ()> {
        Box::pin(self.verify_node_join_enrollment(approval))
    }
    fn reply<'a>(
        &'a self,
        offer: &'a iroh_chain::PairingOffer,
        bearer: &'a str,
        approval: &'a of_model::node_join::NodeJoinApproval,
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
            of_model::id::Id::now_v7()
        ));
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("reserve unavailable SQL endpoint");
        let address = listener.local_addr().expect("SQL endpoint");
        drop(listener);
        let mut config = crate::config::app_config::AppConfig {
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
        assert!(crate::runtime::identity_client(&config).await.is_err());
        assert!(!root.exists());
        config
            .identity_database
            .as_mut()
            .expect("remote config")
            .endpoint = "http://sql.example:50051".into();
        assert!(crate::runtime::identity_client(&config).await.is_err());
        assert!(!root.exists());
    }
}
