use std::{
    fs, io,
    path::Path,
    sync::{Arc, OnceLock},
    time::Duration,
};

use api::serve_listener;
use axum::Router;
use db::Database;
use device_service::HostedControlPlane;
use device_service::hosted_join::NodeJoinApproval;
use idp_model::model::Id;
use idp_server::{IdpRuntime, OwnerProvisioner, build_runtime as build_idp_runtime};
use iroh::protocol::Router as IrohRouter;
use management_service::LocalPermissions;
use storage_server::{
    IdpClient, LocalReplicationAuthorizer, RouterState as StorageRouterState, StorageRuntime,
    build_runtime as build_storage_runtime,
};
use tokio::{net::TcpListener, task::JoinHandle, time::sleep};
use tokio_util::sync::CancellationToken;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

use crate::{SecurityContext, SecurityProtocol, UnifiedEndpoint, config::UnifiedConfig};

const IDP_PREFIX: &str = "/idp";
const MANAGEMENT_PREFIX: &str = "/management";
const DEVICE_PREFIX: &str = "/device";
const STORAGE_PREFIX: &str = "/storage";
const PEER_REFRESH_RETRY_DELAY: Duration = Duration::from_secs(2);

pub(crate) fn storage_introspection_error(
    error: idp_model::contract::ErrorResponse,
) -> storage_server::IdpIntrospectionError {
    use idp_model::contract::ErrorCode;
    use storage_server::IdpIntrospectionError;

    match error.error {
        ErrorCode::ServerError | ErrorCode::TemporarilyUnavailable => {
            IdpIntrospectionError::ServiceUnavailable
        }
        _ => IdpIntrospectionError::InvalidToken,
    }
}

pub(crate) fn local_storage_idp_client(idp: Arc<IdpRuntime>) -> IdpClient {
    IdpClient::local(move |token| {
        let idp = Arc::clone(&idp);
        async move {
            idp.authorize_storage_bearer(&token)
                .await
                .map_err(storage_introspection_error)
        }
    })
}

pub(crate) fn owner_replication_authorizer(
    authority: device_server::DeviceAuthority,
    local_endpoint: iroh::EndpointId,
    copies: Arc<OnceLock<storage_server::StorageCopyRetirement>>,
) -> LocalReplicationAuthorizer {
    let selected = authority.clone();
    let copies = Arc::downgrade(&copies);
    LocalReplicationAuthorizer::from_callbacks(
        move |endpoint| {
            let authority = selected.clone();
            async move {
                authority
                    .selected_resources(&endpoint.to_string())
                    .await
                    .map_err(|error| format!("{error:?}"))
            }
        },
        move |pending| {
            let authority = authority.clone();
            async move {
                authority
                    .acknowledge_owner_cleanup(&pending)
                    .await
                    .map_err(|error| format!("{error:?}"))
            }
        },
        Some((local_endpoint, CancellationToken::new())),
        move || {
            copies
                .upgrade()
                .ok_or("Storage runtime was released")?
                .get()
                .cloned()
                .ok_or_else(|| "Storage cleanup is not ready".into())
        },
    )
}

/// Already initialized owners retained by a bundled host from provisioning into serving.
pub struct RuntimeOwners {
    pub endpoint: Arc<UnifiedEndpoint>,
    pub idp: Arc<Database>,
    pub devices: Arc<Database>,
}

impl RuntimeOwners {
    pub(crate) async fn open(config: &UnifiedConfig) -> io::Result<Self> {
        let endpoint =
            Arc::new(UnifiedEndpoint::open(&config.endpoint_data_dir, &config.idp).await?);
        let result = async {
            let idp =
                idp_server::open_local_identity_database(Path::new(&config.idp.data_dir)).await?;
            let management = device_server::open_device_database(&config.device).await?;
            Ok(Self {
                endpoint: Arc::clone(&endpoint),
                idp,
                devices: management,
            })
        }
        .await;
        if result.is_err() {
            endpoint.close().await;
        }
        result
    }
}

pub struct UnifiedRuntime {
    listener: Option<TcpListener>,
    listener_addr: std::net::SocketAddr,
    router: Router,
    endpoint: Arc<UnifiedEndpoint>,
    pub(crate) api_base: String,
    pub(crate) management_control_plane: Arc<HostedControlPlane>,
    protocol_router: Option<IrohRouter>,
    storage_runtime: Option<StorageRuntime>,
    cancellation_token: CancellationToken,
    peer_refresh: Option<JoinHandle<()>>,
    peer_changes: ofdb_sql::ChangeSubscription,
    security_protocol: SecurityProtocol,
    owner: Arc<OwnerProvisioner>,
    node_joins: Arc<device_server::NodeJoinSource>,
    join_processing: Option<JoinHandle<io::Result<()>>>,
    _managed_copies: Arc<OnceLock<storage_server::StorageCopyRetirement>>,
    #[cfg(test)]
    pub(crate) device_engine: Arc<Database>,
}

impl UnifiedRuntime {
    pub async fn build(config: UnifiedConfig) -> io::Result<Self> {
        let listener = TcpListener::bind(config.listen_addr).await?;
        let listener_addr = listener.local_addr()?;
        Self::build_on_listener(config, listener, loopback_base(listener_addr)).await
    }

    /// Opens an already provisioned host; this is not a fresh setup boundary.
    /// The host must durably authorize fresh intent, retain its exact endpoint/User/Client
    /// identities, call `UnifiedEndpoint::open_fresh` once, and complete the owner's exact
    /// secret inventory before initial private production. Retries use existing opening;
    /// missing endpoint keys or preparation evidence require recovery, not fresh fallback.
    /// The unified CLI uses `LocalProvisioner` for this explicit local boundary.
    pub async fn build_https(config: UnifiedConfig) -> io::Result<Self> {
        let api_base = crate::LocalTls::api_base(config.listen_addr)?;
        let listener = TcpListener::bind(config.listen_addr).await?;
        Self::build_on_listener(config, listener, api_base).await
    }

    pub async fn build_on_listener(
        config: UnifiedConfig,
        listener: TcpListener,
        api_base: String,
    ) -> io::Result<Self> {
        let address = listener.local_addr()?;
        Box::pin(Self::build_hosted(
            config,
            Some(listener),
            address,
            api_base,
            None,
        ))
        .await
    }

    pub(crate) async fn build_hosted_https(config: UnifiedConfig) -> io::Result<Self> {
        let address = config.listen_addr;
        let base = crate::LocalTls::api_base(address)?;
        Box::pin(Self::build_hosted(config, None, address, base, None)).await
    }

    pub async fn build_with_owners(
        config: UnifiedConfig,
        listener: Option<TcpListener>,
        api_base: String,
        owners: RuntimeOwners,
    ) -> io::Result<Self> {
        let address = match &listener {
            Some(listener) => listener.local_addr()?,
            None => config.listen_addr,
        };
        Self::build_hosted(config, listener, address, api_base, Some(owners)).await
    }

    async fn build_hosted(
        config: UnifiedConfig,
        listener: Option<TcpListener>,
        listener_addr: std::net::SocketAddr,
        api_base: String,
        owners: Option<RuntimeOwners>,
    ) -> io::Result<Self> {
        validate_config(&config)?;
        validate_data_dirs(&config)?;
        if !listener_addr.ip().is_loopback() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unified listener must bind to loopback; use a TLS-terminating reverse proxy for external access",
            ));
        }
        validate_api_base(&api_base, listener_addr)?;
        let cancellation_token = CancellationToken::new();
        let RuntimeOwners {
            endpoint,
            idp: idp_engine,
            devices: device_engine,
        } = match owners {
            Some(owners) => owners,
            None => RuntimeOwners::open(&config).await?,
        };
        let mut pending_storage = None;
        let result: io::Result<Self> =
            async {
                let server = endpoint.server().clone();
                let local_permissions = LocalPermissions::new(idp_engine.client());
                let devices = Arc::new(device_service::replica::DbDeviceRepo::new(
                    Arc::clone(&device_engine).client(),
                ));
                let mut idp_config = config.idp.clone();
                idp_config.server.prefix = Some(IDP_PREFIX.to_owned());
                idp_config.control_plane_uri = None;
                let idp_runtime = Arc::new(
                    build_idp_runtime(
                        &idp_config,
                        idp_engine.client(),
                        Some(Arc::clone(&devices)),
                        Arc::new(endpoint.identity()?),
                        server.clone(),
                        Some(local_permissions),
                    )
                    .await?,
                );

                let idp_api_base = format!("{api_base}{IDP_PREFIX}/");
                let storage_api_base = format!("{api_base}{STORAGE_PREFIX}/");
                let management_control_plane = build_control_plane(
                    Arc::clone(&idp_runtime),
                    devices,
                    &idp_api_base,
                    &storage_api_base,
                    &idp_config.oauth2.issuer,
                )?;
                let management_control_plane = Arc::new(
                    management_control_plane
                        .with_device_identity(Arc::new(endpoint.identity()?))
                        .with_pairing_acceptance(idp_runtime.pairing_acceptance()),
                );

                let mut device_config = config.device.clone();
                device_config.api_public_uri = format!("{api_base}{DEVICE_PREFIX}");
                device_config.server.prefix = Some(DEVICE_PREFIX.to_owned());
                let mut management_config = config.management.clone();
                management_config.server.prefix = Some(MANAGEMENT_PREFIX.to_owned());
                let mut storage_config = config.storage.clone();
                storage_config.server.prefix = Some(STORAGE_PREFIX.to_owned());
                let copies = Arc::new(OnceLock::new());
                let authorizer = owner_replication_authorizer(
                    device_server::DeviceAuthority::new(
                        Arc::clone(&device_engine),
                        Arc::clone(&management_control_plane),
                    ),
                    server.endpoint().id(),
                    Arc::clone(&copies),
                );
                let storage_state = StorageRouterState::new(&storage_config.api_public_uri)
                    .with_idp_client(local_storage_idp_client(Arc::clone(&idp_runtime)))
                    .with_local_replication_authorizer(authorizer);
                let storage_runtime = build_storage_runtime(
                    storage_state,
                    Path::new(&storage_config.data_dir),
                    STORAGE_PREFIX,
                    "",
                    Some(server.clone()),
                    cancellation_token.clone(),
                )?;

                pending_storage = Some(storage_runtime);
                let storage_runtime = pending_storage.as_ref().expect("Storage was just built");
                copies
                    .set(storage_runtime.copy_retirement().ok_or_else(|| {
                        io::Error::other("local Storage retirement is unavailable")
                    })?)
                    .map_err(|_| io::Error::other("Storage retirement was already initialized"))?;

                let owner = Arc::new(OwnerProvisioner::from_database(
                    &idp_config,
                    Arc::clone(&idp_engine),
                )?);
                let peer_changes = idp_engine
                    .sync_changes()?
                    .merge(device_engine.sync_changes()?);
                let security_protocol = SecurityProtocol::new(
                    server.clone(),
                    Arc::clone(&idp_engine),
                    Arc::clone(&device_engine),
                    Arc::clone(&idp_engine),
                    idp_config.oauth2.issuer.clone(),
                    idp_config.oauth2.local_signer.clone(),
                    Path::new(&config.endpoint_data_dir).join("security-sync-staging"),
                    cancellation_token.clone(),
                );
                endpoint
                    .refresh_approved_peers(&management_control_plane)
                    .await?;
                if storage_runtime.data_handler().is_none() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Storage replication handler is unavailable in unified mode",
                    ));
                }
                let readiness = {
                    let security = security_protocol.clone();
                    let owner = owner.clone();
                    Arc::new(
                        move || -> device_service::hosted_join::JoinFuture<'static, bool> {
                            let security = security.clone();
                            let owner = owner.clone();
                            Box::pin(async move {
                                security.local_node_public_ready(owner.as_ref()).await
                            })
                        },
                    )
                };
                let node_joins = Arc::new(device_server::NodeJoinSource::new(
                    server.clone(),
                    idp_runtime.clone(),
                    management_control_plane.clone(),
                    readiness,
                    device_engine.clone(),
                    idp_config.oauth2.issuer.clone(),
                    cancellation_token.clone(),
                ));
                let management_router = management_server::build_router(
                    idp_engine.client(),
                    &management_config.api_public_uri,
                    MANAGEMENT_PREFIX,
                    Arc::clone(&management_control_plane),
                )
                .await
                .map_err(io::Error::other)?;
                let device_router = device_server::build_router(
                    Arc::clone(&idp_engine).client(),
                    Arc::clone(&device_engine).client(),
                    &format!("{api_base}{DEVICE_PREFIX}"),
                    DEVICE_PREFIX,
                    &device_config.storage_audience,
                    Arc::clone(&management_control_plane),
                    Some(node_joins.clone()),
                )
                .await
                .map_err(io::Error::other)?;
                let router = compose_router(
                    idp_runtime.router(),
                    management_router,
                    device_router,
                    storage_runtime.router(),
                )
                .layer(CorsLayer::very_permissive().allow_private_network(true))
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new().gzip(idp_config.server.gzip));
                Ok(Self {
                    listener,
                    listener_addr,
                    router,
                    endpoint: Arc::clone(&endpoint),
                    api_base,
                    management_control_plane,
                    protocol_router: None,
                    storage_runtime: pending_storage.take(),
                    cancellation_token: cancellation_token.clone(),
                    peer_refresh: None,
                    peer_changes,
                    security_protocol,
                    owner,
                    node_joins,
                    join_processing: None,
                    _managed_copies: copies,
                    #[cfg(test)]
                    device_engine,
                })
            }
            .await;
        if result.is_err() {
            cancellation_token.cancel();
            if let Some(storage_runtime) = pending_storage.take() {
                let _ = storage_runtime.shutdown().await;
            }
            endpoint.close().await;
        }
        result
    }

    pub async fn node_public_ready(&self) -> io::Result<bool> {
        self.security_protocol
            .local_node_public_ready(self.owner.as_ref())
            .await
    }

    pub(crate) fn readiness(&self) -> (SecurityProtocol, Arc<OwnerProvisioner>) {
        (self.security_protocol.clone(), Arc::clone(&self.owner))
    }

    pub(crate) fn join_owner(&self) -> &OwnerProvisioner {
        self.owner.as_ref()
    }

    pub async fn node_join_public_ready(&self) -> io::Result<bool> {
        self.security_protocol
            .node_join_public_ready(self.owner.as_ref())
            .await
    }

    pub async fn node_join_accepting(
        &self,
        bearer: &str,
        accepting: Option<bool>,
    ) -> io::Result<bool> {
        self.node_joins.accepting(bearer, accepting).await
    }

    pub async fn pending_node_joins(&self, bearer: &str) -> io::Result<serde_json::Value> {
        self.node_joins.pending(bearer).await
    }

    pub async fn approve_pending_node_join(
        &self,
        bearer: &str,
        id: Id,
    ) -> io::Result<NodeJoinApproval> {
        idp_server::local_provisioning::provisioning_stage(|| self.node_joins.approve(bearer, id))
            .await
    }

    pub async fn reject_pending_node_join(&self, bearer: &str, id: Id) -> io::Result<()> {
        self.node_joins.reject(bearer, id).await
    }

    pub async fn synchronize_security(
        &self,
        connection: &iroh::endpoint::Connection,
        context: SecurityContext,
    ) -> io::Result<ofdb_sql::SyncResult> {
        self.security_protocol
            .synchronize(connection, context)
            .await
    }

    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.listener_addr
    }

    pub fn take_listener(&mut self) -> io::Result<TcpListener> {
        self.listener.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Unified runtime listener was already taken",
            )
        })
    }

    pub fn endpoint_id(&self) -> iroh::EndpointId {
        self.endpoint.endpoint_id()
    }

    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub async fn start_background_tasks(&mut self) -> io::Result<()> {
        if self.cancellation_token.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "unified background startup was cancelled",
            ));
        }
        if self.peer_refresh.is_some() {
            return Ok(());
        }
        if !self.node_public_ready().await? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "current local Node readiness is required before background startup",
            ));
        }
        let peer_changes = self.peer_changes.clone();
        let policy_changes = self.peer_changes.clone();
        if let Err(error) = self
            .endpoint
            .refresh_approved_peers(&self.management_control_plane)
            .await
        {
            self.endpoint.refresh_admission([]);
            return Err(error);
        }
        if self.cancellation_token.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "unified background startup was cancelled",
            ));
        }
        let storage_runtime = self
            .storage_runtime
            .as_mut()
            .ok_or_else(|| io::Error::other("unified Storage runtime is unavailable"))?;
        self.protocol_router = Some(
            compose_protocol_router(
                self.endpoint.server(),
                storage_runtime,
                &self.security_protocol,
            )
            .ok_or_else(|| {
                io::Error::other("unified Storage replication handler is unavailable")
            })?,
        );
        storage_runtime.set_policy_changes(policy_changes);
        storage_runtime.start_background_tasks()?;
        let source = self.node_joins.clone();
        self.join_processing = Some(tokio::spawn(async move { source.run().await }));
        self.peer_refresh = Some(spawn_peer_refresh(
            Arc::clone(&self.endpoint),
            Arc::clone(&self.management_control_plane),
            peer_changes,
            self.cancellation_token.clone(),
        ));
        Ok(())
    }

    pub async fn serve(mut self) -> io::Result<()> {
        if self.api_base.starts_with("https://") {
            self.shutdown().await?;
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "HTTPS runtime requires serve_https; plaintext fallback is forbidden",
            ));
        }
        let listener = match self.take_listener() {
            Ok(listener) => listener,
            Err(error) => {
                let _ = self.shutdown().await;
                return Err(error);
            }
        };
        let serve_task = tokio::spawn(serve_listener(
            self.router.clone(),
            listener,
            self.cancellation_token.clone(),
        ));
        self.finish_serving(serve_task).await
    }

    #[cfg(test)]
    pub(crate) fn node_join_source_for_test(&self) -> Arc<device_server::NodeJoinSource> {
        self.node_joins.clone()
    }

    pub async fn serve_https(mut self, tls: crate::LocalTls) -> io::Result<()> {
        if !self.api_base.starts_with("https://") {
            self.shutdown().await?;
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "HTTPS runtime must be built with an HTTPS API base",
            ));
        }
        let listener = match self.take_listener() {
            Ok(listener) => listener,
            Err(error) => {
                let _ = self.shutdown().await;
                return Err(error);
            }
        };
        let listener = tls.listener(listener);
        let router = self.router.clone();
        let cancellation = self.cancellation_token.clone();
        let serve_task = tokio::spawn(async move {
            axum::serve(listener, router.into_make_service())
                .with_graceful_shutdown(cancellation.cancelled_owned())
                .await
        });
        self.finish_serving(serve_task).await
    }

    async fn finish_serving(mut self, serve_task: JoinHandle<io::Result<()>>) -> io::Result<()> {
        if let Err(error) = wait_until_ready(&self.api_base, &self.cancellation_token).await {
            self.cancellation_token.cancel();
            let _ = serve_task.await;
            let _ = self.shutdown().await;
            return Err(error);
        }
        if let Err(error) = self.start_background_tasks().await {
            self.cancellation_token.cancel();
            let _ = serve_task.await;
            let _ = self.shutdown().await;
            return Err(error);
        }

        let serve_result = match serve_task.await {
            Ok(result) => result,
            Err(error) => Err(io::Error::other(error)),
        };
        let shutdown_result = self.shutdown().await;
        serve_result.and(shutdown_result)
    }

    pub async fn shutdown(mut self) -> io::Result<()> {
        self.cancellation_token.cancel();
        self.endpoint.server().set_pairing_enabled(false);
        let mut shutdown_error = None;
        if let Some(task) = self.join_processing.take() {
            match task.await {
                Ok(Ok(())) => (),
                Ok(Err(error)) => {
                    shutdown_error = Some(error);
                }
                Err(error) => {
                    shutdown_error = Some(io::Error::other(error));
                }
            }
        }
        self.node_joins.clear().await;
        if let Some(peer_refresh) = self.peer_refresh.take()
            && let Err(error) = peer_refresh.await
        {
            shutdown_error = Some(io::Error::other(error));
        }
        if let Some(storage_runtime) = self.storage_runtime.take()
            && let Err(error) = storage_runtime.shutdown().await
        {
            shutdown_error.get_or_insert(error);
        }
        if let Some(protocol_router) = self.protocol_router.take() {
            if let Err(error) = protocol_router.shutdown().await {
                shutdown_error.get_or_insert_with(|| io::Error::other(error));
            }
        } else {
            self.endpoint.close().await;
        }
        match shutdown_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

pub(crate) fn validate_config(config: &UnifiedConfig) -> io::Result<()> {
    crate::config::validate_config(config)
}

fn validate_data_dirs(config: &UnifiedConfig) -> io::Result<()> {
    let directories = [
        Path::new(&config.idp.data_dir),
        Path::new(&config.device.data_dir),
        Path::new(&config.storage.data_dir),
        config.endpoint_data_dir.as_path(),
    ];
    let mut canonical = Vec::with_capacity(directories.len());
    for directory in directories {
        fs::create_dir_all(directory)?;
        let path = fs::canonicalize(directory)?;
        if canonical.iter().any(|existing: &std::path::PathBuf| {
            existing.starts_with(&path) || path.starts_with(existing)
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "IdP, Management, Storage and unified endpoint data directories must not overlap",
            ));
        }
        canonical.push(path);
    }
    Ok(())
}

pub(crate) fn build_control_plane(
    idp: Arc<IdpRuntime>,
    devices: Arc<device_service::replica::DbDeviceRepo>,
    idp_api_base: &str,
    storage_api_base: &str,
    issuer: &str,
) -> io::Result<HostedControlPlane> {
    let inspect = Arc::clone(&idp);
    Ok(
        HostedControlPlane::new_with_services(idp_api_base, storage_api_base, issuer)
            .map_err(invalid_config)?
            .with_local_devices(devices)
            .with_local_idp(
                move |token| {
                    let idp = Arc::clone(&inspect);
                    async move { idp.authorize_management_bearer(&token).await }
                },
                move |user_id| {
                    let idp = Arc::clone(&idp);
                    async move { idp.active_user(user_id).await }
                },
            ),
    )
}

fn validate_api_base(api_base: &str, listener_addr: std::net::SocketAddr) -> io::Result<()> {
    let url = reqwest::Url::parse(api_base).map_err(|error| invalid_config(error.to_string()))?;
    let loopback_host = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    });
    if !matches!(url.scheme(), "http" | "https")
        || !loopback_host
        || url.port() != Some(listener_addr.port())
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid_config(
            "unified API base must use HTTP(S) on the supplied loopback listener without a path, credentials, query, or fragment".to_owned(),
        ));
    }
    Ok(())
}

fn loopback_base(address: std::net::SocketAddr) -> String {
    let loopback = match address.ip() {
        std::net::IpAddr::V4(_) => std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        std::net::IpAddr::V6(_) => std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
    };
    format!(
        "http://{}",
        std::net::SocketAddr::new(loopback, address.port())
    )
}

async fn wait_until_ready(
    api_base: &str,
    cancellation_token: &CancellationToken,
) -> io::Result<()> {
    let client = crate::LocalTls::verification_client()?;
    let owner_base = format!("{api_base}{IDP_PREFIX}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        tokio::select! {
            () = cancellation_token.cancelled() => {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "unified startup was cancelled"));
            }
            response = crate::bootstrap_verification::check_owner_health(&owner_base, "idp", &client) => {
                if response.is_ok() {
                    return Ok(());
                }
            }
            () = sleep(Duration::from_millis(50)) => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "unified HTTP listener did not become ready",
            ));
        }
    }
}

fn invalid_config(error: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}

fn spawn_peer_refresh(
    endpoint: Arc<UnifiedEndpoint>,
    management_control_plane: Arc<HostedControlPlane>,
    changes: ofdb_sql::ChangeSubscription,
    cancellation_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        run_peer_refresh(changes, cancellation_token, || async {
            let result = endpoint
                .refresh_approved_peers(&management_control_plane)
                .await;
            if let Err(error) = &result {
                log::warn!("failed to refresh unified Iroh peer admission: {error}");
                endpoint.refresh_admission([]);
            }
            result
        })
        .await;
        endpoint.refresh_admission([]);
    })
}

async fn run_peer_refresh<F, Fut>(
    mut changes: ofdb_sql::ChangeSubscription,
    cancellation_token: CancellationToken,
    mut refresh: F,
) where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = io::Result<()>>,
{
    let mut retry = false;
    loop {
        tokio::select! {
            biased;
            () = cancellation_token.cancelled() => return,
            result = changes.changed() => if result.is_err() { return; },
            () = sleep(PEER_REFRESH_RETRY_DELAY), if retry => {},
        }

        tokio::select! {
            biased;
            () = cancellation_token.cancelled() => return,
            result = refresh() => retry = result.is_err(),
        }
    }
}

#[cfg(test)]
#[path = "runtime_peer_refresh_tests.rs"]
mod peer_refresh_tests;

#[cfg(test)]
#[path = "../../../tests/support/identity.rs"]
mod identity_fixture;

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::{IpAddr, Ipv4Addr, SocketAddr},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::identity_fixture::{run_permission_http_checks, seed_idp};
    use axum::{body::Body, http::Request};
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    use crate::UnifiedConfig;

    use super::{UnifiedRuntime, loopback_base, validate_api_base, validate_config};

    #[test]
    fn host_api_base_must_match_the_loopback_listener() {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4321);
        assert!(validate_api_base("https://localhost:4321", address).is_ok());
        assert!(validate_api_base("http://127.0.0.1:4321", address).is_ok());
        assert!(validate_api_base("https://example.com:4321", address).is_err());
        assert!(validate_api_base("https://localhost:4322", address).is_err());
        assert!(validate_api_base("https://localhost:4321/idp", address).is_err());
    }

    #[test]
    fn builds_three_prefixed_services_with_one_endpoint() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .thread_stack_size(64 * 1024 * 1024)
                    .enable_all()
                    .build()
                    .expect("build test runtime")
                    .block_on(run_unified_listener_acceptance());
            })
            .expect("spawn large-stack unified acceptance test")
            .join()
            .expect("unified acceptance test thread did not panic");
    }

    async fn approve_test_peer(
        root: &std::path::Path,
        config: &idp_server::AppConfig,
        approved_peer_id: iroh::EndpointId,
        device_only_peer: iroh::EndpointId,
    ) -> idp_service::oauth2::NodeSignerBinding {
        use device_service::DeviceRepo;
        use idp_service::repo::UserRepo;

        let issuer = &config.oauth2.issuer;
        let engine =
            idp_server::open_local_identity_database(std::path::Path::new(&config.data_dir))
                .await
                .expect("open native trust fixture");
        let users = idp_service::replica::DbUserRepo::new(engine.client(), config.password.clone());
        let owner = users
            .find_user_by_username_or_email("administrator")
            .await
            .expect("local owner lookup")
            .expect("fixture active owner");
        let management = device_server::open_local_device_database(&root.join("device"))
            .await
            .expect("Management Device fixture");
        let devices = device_service::replica::DbDeviceRepo::new(management.client());
        let approved = devices
            .find_approved_by_public_key(&approved_peer_id.to_string())
            .await
            .expect("lookup approved peer")
            .expect("approved peer Device");
        let unsigned = devices
            .create(
                owner.id.to_string(),
                "device-without-signer".into(),
                device_only_peer.to_string(),
                "".into(),
                vec![],
                i64::MAX,
            )
            .await
            .expect("enroll Device without signer");
        devices
            .approve(unsigned.id, &[])
            .await
            .expect("approve Device without signer")
            .expect("pending Device has exact unexpired enrollment");
        let binding = idp_service::oauth2::NodeSignerBinding {
            member_id: approved.id,
            key_id: idp_model::model::Id::now_v7(),
            endpoint_id: approved_peer_id.to_string(),
        };
        let mut peer_config = config.clone();
        peer_config.data_dir = root
            .join("independent-peer-idp")
            .to_string_lossy()
            .into_owned();
        let peer = idp_server::OwnerProvisioner::open(&peer_config)
            .await
            .expect("independent peer signer owner with fixture secret store");
        peer.prepare_fresh_setup_secret_inventory(idp_model::model::Id::now_v7(), &[])
            .await
            .expect("fresh independent signer inventory");
        peer.prepare_setup_client_slots(&[])
            .await
            .expect("complete peer inventory before private signing");
        let public = peer
            .prepare_node_signer(&binding)
            .expect("independent peer private preparation");
        idp_service::replica::DbReplicaSignerRepo::new(engine.client())
            .enroll(
                &idp_model::contract::ReplicaMembership {
                    installation_id: issuer.clone(),
                    issuer: issuer.clone(),
                    member_id: binding.member_id,
                    endpoint_id: binding.endpoint_id.clone(),
                    approved_at: 1,
                    revoked_at: None,
                },
                &idp_model::contract::IdpSignerRecord {
                    member_id: binding.member_id,
                    key_id: binding.key_id,
                    issuer: issuer.clone(),
                    public_jwk: public,
                    approved_at: Some(1),
                    revoked_at: None,
                    expires_at: None,
                },
            )
            .await
            .expect("enroll independent peer signer separately from Device approval");
        binding
    }

    #[tokio::test]
    async fn native_membership_restores_offline_and_learned_revocation_removes_admission() {
        let root = std::env::temp_dir().join(format!(
            "native-admission-{}",
            idp_model::model::Id::now_v7()
        ));
        let mut config = idp_server::AppConfig {
            data_dir: root.join("idp").to_string_lossy().into_owned(),
            ..idp_server::AppConfig::default()
        };
        config.oauth2.issuer = format!("https://{}.test/idp", idp_model::model::Id::now_v7());
        let endpoint_dir = root.join("endpoint");
        let initial = crate::UnifiedEndpoint::open_fresh(&endpoint_dir, &config)
            .await
            .expect("persist Node endpoint");
        let local = initial.endpoint_id();
        let peer = iroh::SecretKey::generate().public();
        let device_only = iroh::SecretKey::generate().public();
        let fixture = seed_idp(
            &root,
            &config.oauth2.issuer,
            &local.to_string(),
            &peer.to_string(),
        )
        .await;
        config.oauth2.local_signer = Some(fixture.local_signer);
        let binding = approve_test_peer(&root, &config, peer, device_only).await;
        initial.close().await;
        drop(initial);
        for revoked in [false, true] {
            let endpoint = std::sync::Arc::new(
                crate::UnifiedEndpoint::open(&endpoint_dir, &config)
                    .await
                    .expect("offline endpoint reopen"),
            );
            assert_eq!(endpoint.endpoint_id(), local);
            let engine =
                idp_server::open_local_identity_database(std::path::Path::new(&config.data_dir))
                    .await
                    .expect("native trust reopen");
            let management = device_server::open_local_device_database(&root.join("device"))
                .await
                .expect("reopen Management Device owner");
            let devices = std::sync::Arc::new(device_service::replica::DbDeviceRepo::new(
                management.clone().client(),
            ));
            let idp = idp_server::build_runtime(
                &config,
                engine.client(),
                Some(devices.clone()),
                std::sync::Arc::new(endpoint.identity().expect("restored identity")),
                endpoint.server().clone(),
                None,
            )
            .await
            .expect("offline signer and trust runtime");
            let idp = std::sync::Arc::new(idp);
            let control = std::sync::Arc::new(
                super::build_control_plane(
                    idp.clone(),
                    devices,
                    "http://127.0.0.1:1",
                    "http://127.0.0.1:1",
                    &config.oauth2.issuer,
                )
                .expect("local Management authority"),
            );
            endpoint
                .refresh_approved_peers(&control)
                .await
                .expect("restore approved Device admission");
            assert_eq!(endpoint.server().peers().contains(peer), !revoked);
            assert!(endpoint.server().peers().contains(local));
            assert!(
                endpoint.server().peers().contains(device_only),
                "approved Device needs no token signer for endpoint admission"
            );
            if !revoked {
                use idp_service::repo::UserRepo;
                let users =
                    idp_service::replica::DbUserRepo::new(engine.client(), config.password.clone());
                let owner = users
                    .find_user_by_username_or_email("administrator")
                    .await
                    .expect("local owner lookup")
                    .expect("fixture active owner");
                let member = config
                    .oauth2
                    .local_signer
                    .as_ref()
                    .expect("local binding")
                    .member_id;

                assert_eq!(
                    control
                        .get_storage_endpoint_identity(&local.to_string())
                        .await
                        .expect("current conflict-clear approved Device and active owner")
                        .owner_subject,
                    owner.id.to_string()
                );
                let policies = device_service::replica::DbSelectionPolicyRepo::new(
                    std::sync::Arc::clone(&management).client(),
                );
                let copies = std::sync::Arc::new(std::sync::OnceLock::new());
                assert!(
                    copies
                        .set(storage_server::StorageCopyRetirement::new(
                            std::sync::Arc::new(
                                storage_service::DatabaseRuntime::new(root.join("databases"))
                                    .expect("native database runtime")
                            ),
                            std::sync::Arc::new(
                                storage_service::ScopedFileSystemRuntime::new(
                                    root.join("files"),
                                    local
                                )
                                .expect("native filesystem runtime")
                            ),
                        ))
                        .is_ok()
                );
                let authorizer = super::owner_replication_authorizer(
                    device_server::DeviceAuthority::new(
                        management.clone(),
                        std::sync::Arc::clone(&control),
                    ),
                    local,
                    std::sync::Arc::clone(&copies),
                );
                assert!(
                    authorizer.selected_resources(local).await.is_err(),
                    "missing local policy fails closed"
                );
                policies
                    .set_prevalidated(device_service::replica::SelectionPolicy {
                        device_id: member,
                        owner_subject: owner.id.to_string(),
                        application_id: None,
                        selected_kind: None,
                        selected_id: None,
                        admin_allowed: true,
                    })
                    .await
                    .expect("persist real empty policy");
                assert!(
                    authorizer
                        .selected_resources(local)
                        .await
                        .expect("production owner-service callback works offline")
                        .is_empty()
                );
                assert_eq!(
                    control
                        .get_storage_endpoint_identity(&device_only.to_string())
                        .await
                        .expect("unsigned approved Device has an active User owner")
                        .owner_subject,
                    owner.id.to_string()
                );
                assert!(
                    authorizer.selected_resources(device_only).await.is_err(),
                    "endpoint approval without exact resource policy cannot authorize resources"
                );
                policies
                    .set_admin_allowed_prevalidated(member, &owner.id.to_string(), false)
                    .await
                    .expect("learn restriction");
                assert!(
                    authorizer.selected_resources(local).await.is_err(),
                    "production callback reads newly learned restrictions"
                );
                idp_service::replica::DbReplicaSignerRepo::new(engine.client())
                    .revoke(binding.member_id, &config.oauth2.issuer, 2)
                    .await
                    .expect("learn signer-only revocation");
                endpoint
                    .refresh_approved_peers(&control)
                    .await
                    .expect("refresh approval after signer revocation");
                assert!(
                    endpoint.server().peers().contains(peer),
                    "signer revocation does not revoke Device approval"
                );
                assert!(endpoint.server().peers().contains(device_only));
                assert_eq!(
                    control
                        .get_storage_endpoint_identity(&peer.to_string())
                        .await
                        .expect("approved Device remains valid without signer trust")
                        .owner_subject,
                    owner.id.to_string()
                );
                let cancellation = tokio_util::sync::CancellationToken::new();
                let refresh = super::spawn_peer_refresh(
                    endpoint.clone(),
                    control.clone(),
                    engine
                        .sync_changes()
                        .expect("IdP subscription")
                        .merge(management.sync_changes().expect("Management subscription")),
                    cancellation.clone(),
                );
                management
                    .client()
                    .translate_and_execute(
                        &format!(
                            "UPDATE devices SET revoked_at = 2 WHERE id = CAST('{}' AS UUID)",
                            binding.member_id
                        ),
                        &db::SqlTranslator,
                    )
                    .await
                    .expect("learn timestamp-only Device revocation while state remains Approved");
                tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    while endpoint.server().peers().contains(peer) {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect("committed Device revocation removes admission without explicit refresh");
                assert!(!endpoint.server().peers().contains(peer));
                assert!(endpoint.server().peers().contains(device_only));
                assert!(
                    control
                        .get_storage_endpoint_identity(&peer.to_string())
                        .await
                        .is_err()
                );
                cancellation.cancel();
                refresh.await.expect("peer refresh shutdown");
            } else {
                use idp_service::repo::UserRepo;
                let users =
                    idp_service::replica::DbUserRepo::new(engine.client(), config.password.clone());
                let owner = users
                    .find_user_by_username_or_email("administrator")
                    .await
                    .expect("reopened owner lookup")
                    .expect("fixture owner remains active through offline reopen");
                users
                    .delete_user_by_id(owner.id)
                    .await
                    .expect("learn owner deletion");
                assert!(
                    control
                        .get_storage_endpoint_identity(&local.to_string())
                        .await
                        .is_err(),
                    "next owner-service call denies deleted User without peer contact"
                );
            }
            endpoint.close().await;
        }
        fs::remove_dir_all(root).expect("remove isolated native Node stores");
    }

    async fn run_unified_listener_acceptance() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after Unix epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("unified-runtime-{}-{unique}", std::process::id()));
        let mut idp = idp_server::AppConfig::default();
        idp.data_dir = root.join("idp").to_string_lossy().into_owned();
        idp.api_public_uri = "https://idp.example".to_owned();
        idp.oauth2.issuer = format!("https://idp-{unique}.example/idp");
        idp.service_audience = Some("idp-services".to_owned());
        let mut management = management_server::AppConfig::default();
        management.data_dir = root.join("device").to_string_lossy().into_owned();
        management.api_public_uri = "https://management.example".to_owned();
        management.storage_audience = "storage-service".to_owned();
        let mut storage = storage_server::AppConfig::default();
        storage.data_dir = root.join("storage").to_string_lossy().into_owned();
        storage.api_public_uri = "https://storage.example".to_owned();
        let mut config = UnifiedConfig {
            listen_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            endpoint_data_dir: root.join("endpoint"),
            idp,
            device: device_server::AppConfig {
                data_dir: root.join("device").to_string_lossy().into_owned(),
                storage_audience: management.storage_audience.clone(),
                ..Default::default()
            },
            management,
            storage,
        };
        let mut invalid = config.clone();
        invalid.idp.service_audience = Some(String::new());
        assert!(validate_config(&invalid).is_err());
        let endpoint = crate::UnifiedEndpoint::open_fresh(&config.endpoint_data_dir, &config.idp)
            .await
            .expect("open endpoint for initial test provisioning");
        let endpoint_id = endpoint.endpoint_id();
        let approved_peer_id = iroh::SecretKey::generate().public();
        let issuer = config.idp.oauth2.issuer.clone();
        let mut fixture = seed_idp(
            &root,
            &issuer,
            &endpoint_id.to_string(),
            &approved_peer_id.to_string(),
        )
        .await;
        config.idp.oauth2.local_signer = Some(fixture.local_signer.clone());
        super::identity_fixture::seed_permissions_at(&root.join("idp/idp.redb"), &mut fixture)
            .await;
        let device_only_peer = iroh::SecretKey::generate().public();
        let peer_binding =
            approve_test_peer(&root, &config.idp, approved_peer_id, device_only_peer).await;
        let persisted_idp_config = config.idp.clone();
        let mut public_context_grant = None;
        {
            let policy = device_server::open_local_device_database(std::path::Path::new(
                &config.device.data_dir,
            ))
            .await
            .expect("policy fixture");
            device_service::replica::DbSelectionPolicyRepo::new(policy.clone().client())
                .set_prevalidated(device_service::replica::SelectionPolicy {
                    device_id: fixture.local_signer.member_id,
                    owner_subject: fixture.administrator.to_string(),
                    application_id: None,
                    selected_kind: None,
                    selected_id: None,
                    admin_allowed: true,
                })
                .await
                .expect("explicit initial local Device policy");
            let policy = idp_server::open_local_identity_database(std::path::Path::new(
                &config.idp.data_dir,
            ))
            .await
            .expect("shared identity policy");
            let service = management_service::ManagementService::new(
                management_service::replica::DbPermissionRepo::new(policy.client()),
                management_service::replica::DbRoleRepo::new(policy.client()),
            );
            let role = service
                .create_role(
                    idp_model::contract::INSTALLATION_POLICY_ID,
                    "Node approval and public contexts",
                    None,
                )
                .await
                .expect("explicit test role");
            service
                .add_role_to_user(
                    idp_model::contract::INSTALLATION_POLICY_ID,
                    fixture.administrator,
                    role.id,
                )
                .await
                .expect("explicit User grant");
            for action in [
                idp_model::contract::IdentityAction::ReplicaSignersEnroll,
                idp_model::contract::IdentityAction::IdpSecuritySync,
                idp_model::contract::IdentityAction::ManagementSecuritySync,
            ] {
                let permission = service
                    .create_permission(
                        idp_model::contract::INSTALLATION_POLICY_ID,
                        action.permission(),
                        None,
                    )
                    .await
                    .expect("exact permission");
                service
                    .add_permission_to_role(
                        idp_model::contract::INSTALLATION_POLICY_ID,
                        role.id,
                        permission.id,
                    )
                    .await
                    .expect("exact role permission");
                if action == idp_model::contract::IdentityAction::IdpSecuritySync {
                    public_context_grant = Some((role.id, permission.id));
                }
            }
        }
        endpoint.close().await;

        let mut early_runtime = UnifiedRuntime::build(config.clone())
            .await
            .expect("build runtime for taken-listener cleanup");
        let early_listener = early_runtime
            .take_listener()
            .expect("take host listener once");
        assert!(
            early_runtime.protocol_router.is_none(),
            "building must not start the owner router or its mesh background work"
        );
        let early_endpoint = early_runtime.endpoint.server().endpoint().clone();
        assert_eq!(
            early_runtime
                .serve()
                .await
                .expect_err("taken listener refuses serve")
                .kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert!(
            early_endpoint.is_closed(),
            "early serve error must close its shared endpoint"
        );
        drop(early_endpoint);
        drop(early_listener);

        let listener = TcpListener::bind(config.listen_addr)
            .await
            .expect("bind host-owned unified listener");
        let api_base = loopback_base(listener.local_addr().expect("read host listener address"));
        let mut runtime = UnifiedRuntime::build_on_listener(config, listener, api_base)
            .await
            .expect("build unified runtime on host-owned listener");
        assert_eq!(runtime.endpoint_id(), endpoint_id);
        assert!(
            runtime.endpoint.server().peers().contains(approved_peer_id),
            "approved peers restored before handler composition without manual refresh"
        );
        assert!(
            runtime.endpoint.server().peers().contains(device_only_peer),
            "approved Device is admitted without a token signer"
        );
        assert!(runtime.endpoint.server().peers().contains(endpoint_id));

        let address = runtime.local_addr();
        assert_ne!(address.port(), 0);
        assert!(fs::metadata(root.join("endpoint/endpoint.key")).is_ok());

        for path in ["/idp/health", "/management/health", "/storage/health"] {
            let response = runtime
                .router()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .body(Body::empty())
                        .expect("build health request"),
                )
                .await
                .expect("call unified service router");
            assert_eq!(response.status(), http::StatusCode::OK, "{path}");
        }
        for path in [
            "/idp/internal/token/validate",
            "/management/internal/replication/admission",
        ] {
            let response = runtime
                .router()
                .oneshot(
                    Request::builder()
                        .method(http::Method::POST)
                        .uri(path)
                        .header("x-internal-service", "untrusted")
                        .body(Body::empty())
                        .expect("build removed internal-route request"),
                )
                .await
                .expect("call removed internal route");
            assert_eq!(response.status(), http::StatusCode::NOT_FOUND, "{path}");
        }
        let response = runtime
            .router()
            .oneshot(
                Request::builder()
                    .uri("/device/replication/devices/untrusted/selections")
                    .header("x-internal-service", "untrusted")
                    .body(Body::empty())
                    .expect("build normal route request"),
            )
            .await
            .expect("call normal Device route");
        assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);

        let policy = runtime
            .owner
            .node_join_database()
            .expect("local identity owner");
        let service = management_service::ManagementService::new(
            management_service::replica::DbPermissionRepo::new(policy.client()),
            management_service::replica::DbRoleRepo::new(policy.client()),
        );
        let (role_id, permission_id) = public_context_grant.expect("exact context grant");
        service
            .remove_permission_from_role(
                idp_model::contract::INSTALLATION_POLICY_ID,
                role_id,
                permission_id,
            )
            .await
            .expect("remove local readiness grant before background startup");
        assert!(
            runtime.start_background_tasks().await.is_err(),
            "background startup must refuse incomplete current local readiness"
        );
        assert!(
            runtime.endpoint.server().peers().contains(approved_peer_id),
            "local context denial must not erase independently trusted peer admission"
        );
        assert!(
            runtime.protocol_router.is_none(),
            "denied startup must not start mesh work"
        );
        service
            .add_permission_to_role(
                idp_model::contract::INSTALLATION_POLICY_ID,
                role_id,
                permission_id,
            )
            .await
            .expect("restore local readiness grant");
        drop(service);
        drop(policy);

        let cancellation_token = runtime.cancellation_token();
        let server_address = runtime.local_addr();
        let serve_task = tokio::spawn(api::serve_listener(
            runtime.router(),
            runtime.take_listener().expect("host listener"),
            cancellation_token.clone(),
        ));
        runtime
            .start_background_tasks()
            .await
            .expect("start composed runtime tasks");
        let protocol_router = runtime
            .protocol_router
            .as_ref()
            .expect("one host protocol router")
            .clone();
        runtime
            .start_background_tasks()
            .await
            .expect("repeated background startup is idempotent");
        let client = reqwest::Client::new();
        for (path, expected_server) in [
            ("/idp/openapi.json", "https://idp.example/idp"),
            (
                "/management/openapi.json",
                "https://management.example/management",
            ),
            ("/storage/openapi.json", "https://storage.example/storage"),
        ] {
            let openapi = get_json(&client, format!("http://{server_address}{path}")).await;
            assert_eq!(openapi["servers"][0]["url"], expected_server, "{path}");
            if path.starts_with("/storage/") {
                assert!(openapi["paths"].get("/databases").is_some());
                assert!(openapi["paths"].get("/storage/databases").is_none());
            }
        }

        for path in ["/idp/health", "/management/health", "/storage/health"] {
            let response = client
                .get(format!("http://{server_address}{path}"))
                .send()
                .await
                .expect("request live unified service");
            assert_eq!(response.status(), http::StatusCode::OK, "{path}");
        }
        let response = client
            .get(format!(
                "http://{server_address}/device/replication/devices/untrusted/selections"
            ))
            .header("x-internal-service", "untrusted")
            .send()
            .await
            .expect("request live Device route with legacy header only");
        assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);

        let bearer = super::identity_fixture::user_token(
            &client,
            &format!("http://{server_address}"),
            "administrator",
            management_service::MANAGEMENT_APPLICATION_URI,
        )
        .await;
        let mut generated = of_device_client::apis::configuration::Configuration {
            base_path: format!("http://{server_address}/device"),
            client: client.clone(),
            ..Default::default()
        };
        assert!(
            of_device_client::apis::node_join_api::node_join_pending(&generated)
                .await
                .is_err(),
            "generated host routes require bearer admission"
        );
        generated.bearer_access_token = Some(bearer.clone());
        of_device_client::apis::node_join_api::node_join_acceptance(&generated)
            .await
            .expect("generated acceptance against actual composed router");
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/qualify-built-node-join.mjs");
        let node_bearer = bearer.clone();
        let status = tokio::task::spawn_blocking(move || {
            std::process::Command::new("node")
                .arg(script)
                .arg(format!("http://{server_address}"))
                .env("OF_TEST_BEARER", node_bearer)
                .status()
                .expect("built Node-join SDK consumer")
        })
        .await
        .expect("Node-join consumer task");
        assert!(status.success());
        assert!(
            runtime
                .node_public_ready()
                .await
                .expect("current initial Node readiness")
        );
        assert!(
            !runtime
                .node_join_public_ready()
                .await
                .expect("initial Node has no join receipt")
        );
        assert!(
            runtime
                .node_join_accepting("forged", Some(true))
                .await
                .is_err()
        );
        assert!(runtime.pending_node_joins("forged").await.is_err());
        let ordinary = super::identity_fixture::user_token(
            &client,
            &format!("http://{server_address}"),
            "unassigned",
            "idp-services",
        )
        .await;
        assert!(
            runtime
                .node_join_accepting(&ordinary, Some(true))
                .await
                .is_err(),
            "ordinary authentication cannot authorize joining"
        );
        assert!(runtime.pending_node_joins(&ordinary).await.is_err());
        assert!(
            runtime
                .node_join_accepting(&bearer, Some(true))
                .await
                .expect("authorized pairing acceptance")
        );
        let joining_config = idp_server::AppConfig {
            data_dir: root.join("joining-idp").to_string_lossy().into_owned(),
            oauth2: idp_service::oauth2::OAuth2Config {
                issuer: issuer.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        let joining = iroh_chain::Server::bind(
            iroh::endpoint::presets::Minimal,
            iroh_chain::EndpointIdStore::default(),
        )
        .await
        .expect("independent joining endpoint");
        joining.set_pairing_enabled(true);
        let joining_owner = idp_server::OwnerProvisioner::open(&joining_config)
            .await
            .expect("joining owner");
        joining_owner
            .prepare_fresh_setup_secret_inventory(fixture.administrator, &[])
            .await
            .expect("fresh pending joining owner inventory");
        joining_owner
            .prepare_setup_client_slots(&[])
            .await
            .expect("complete pending join inventory before private signing");
        let request = joining_owner
            .prepare_node_join(
                &idp_service::oauth2::NodeSignerBinding {
                    member_id: idp_model::model::Id::now_v7(),
                    key_id: idp_model::model::Id::now_v7(),
                    endpoint_id: joining.endpoint().id().to_string(),
                },
                "Pending equal Node",
                runtime.endpoint_id(),
            )
            .expect("independent joining signer");
        let sending = tokio::spawn({
            let joining = joining.clone();
            let address = runtime.endpoint.server().endpoint().addr();
            let request = request.clone();
            async move { request.send(&joining, address).await }
        });
        let pending = tokio::time::timeout(std::time::Duration::from_secs(20), async {
            loop {
                let offers = runtime
                    .pending_node_joins(&bearer)
                    .await
                    .expect("authorized pending display");
                if !offers.as_array().expect("display list").is_empty() {
                    break offers;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("authenticated offer arrives");
        assert_eq!(
            pending[0]["authenticated_endpoint"],
            joining.endpoint().id().to_string()
        );
        assert_eq!(
            pending[0]["request"],
            serde_json::to_value(&request).expect("display exact request")
        );
        let offer_id = pending[0]["offer_id"]
            .as_str()
            .expect("opaque offer ID")
            .parse()
            .expect("offer UUID");
        assert!(
            runtime
                .approve_pending_node_join("forged", offer_id)
                .await
                .is_err()
        );
        assert!(
            runtime
                .approve_pending_node_join(&ordinary, offer_id)
                .await
                .is_err()
        );
        assert!(
            runtime
                .reject_pending_node_join(&ordinary, offer_id)
                .await
                .is_err()
        );
        assert_eq!(
            runtime
                .pending_node_joins(&bearer)
                .await
                .expect("failed auth preserves offer"),
            pending
        );
        let approval = runtime
            .approve_pending_node_join(&bearer, offer_id)
            .await
            .expect("authorized approval of retained transport offer");
        assert_eq!(
            sending.await.expect("sender task").expect("signed reply"),
            approval
        );
        assert!(
            runtime
                .endpoint
                .server()
                .peers()
                .contains(joining.endpoint().id()),
            "approval refreshes admission immediately"
        );
        assert!(
            runtime
                .pending_node_joins(&bearer)
                .await
                .expect("consumed pending list")
                .as_array()
                .expect("list")
                .is_empty()
        );
        assert!(
            runtime
                .approve_pending_node_join(&bearer, offer_id)
                .await
                .is_err(),
            "no replay of consumed offer"
        );
        let rejected_sender = tokio::spawn({
            let joining = joining.clone();
            let address = runtime.endpoint.server().endpoint().addr();
            async move { request.send(&joining, address).await }
        });
        let rejected_id: idp_model::model::Id =
            tokio::time::timeout(std::time::Duration::from_secs(20), async {
                loop {
                    let pending = runtime
                        .pending_node_joins(&bearer)
                        .await
                        .expect("rejection display");
                    if let Some(id) = pending[0]["offer_id"].as_str() {
                        break id.parse().expect("retained rejection offer ID");
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("rejection offer arrives");
        runtime
            .reject_pending_node_join(&bearer, rejected_id)
            .await
            .expect("authorized rejection");
        assert!(
            rejected_sender
                .await
                .expect("rejected sender task")
                .is_err(),
            "rejection cannot produce an approval receipt"
        );
        assert!(
            runtime
                .pending_node_joins(&bearer)
                .await
                .expect("rejected offers removed")
                .as_array()
                .expect("list")
                .is_empty()
        );
        assert!(
            !runtime
                .node_join_accepting(&bearer, Some(false))
                .await
                .expect("disable acceptance")
        );
        joining.close().await;
        drop(joining_owner);

        let management_device_token = issue_client_token(
            &client,
            &server_address,
            "storage-device",
            "storage-device-secret",
            "storage-service",
            "management.device.lookup",
        )
        .await;
        let response = client
            .get(format!(
                "http://{server_address}/device/devices/endpoints/{endpoint_id}"
            ))
            .bearer_auth(&management_device_token)
            .send()
            .await
            .expect("Management endpoint authority through unified listener");
        assert_eq!(response.status(), http::StatusCode::OK);
        let identity: device_service::DeviceEndpointIdentity = response
            .json()
            .await
            .expect("approved Management endpoint identity");
        assert_eq!(identity.endpoint_id, endpoint_id.to_string());

        let storage_token = issue_client_token(
            &client,
            &server_address,
            "storage-device",
            "storage-device-secret",
            "storage-service",
            "management.replication.read",
        )
        .await;

        let response = client
            .get(format!(
                "http://{server_address}/device/replication/devices/{endpoint_id}/selections"
            ))
            .bearer_auth(&storage_token)
            .send()
            .await
            .expect("call authenticated Device API through unified listener");
        let selection_status = response.status();
        let selection_body = response.text().await.expect("read selection response");
        assert_eq!(selection_status, http::StatusCode::OK, "{selection_body}");
        let selected: model::contract::SelectedResourcesResponse =
            serde_json::from_str(&selection_body).expect("parse selection response");
        assert_eq!(
            selected.device_id,
            fixture.local_signer.member_id.to_string()
        );
        assert_eq!(selected.endpoint_id, endpoint_id.to_string());
        assert_eq!(selected.owner_subject, fixture.administrator.to_string());
        assert!(selected.admin_allowed);
        assert!(selected.resources.is_empty());

        run_permission_http_checks(
            &client,
            &format!("http://{server_address}"),
            &format!("http://{server_address}"),
            &fixture,
            http::StatusCode::FORBIDDEN,
        )
        .await;

        let idp_metadata = get_json(
            &client,
            format!("http://{server_address}/idp/.well-known/openid-configuration"),
        )
        .await;
        assert_eq!(idp_metadata["issuer"], issuer);
        assert_eq!(
            idp_metadata["token_endpoint"],
            format!("{issuer}/oauth2/token")
        );

        let policy = runtime
            .owner
            .node_join_database()
            .expect("local identity owner");
        let service = management_service::ManagementService::new(
            management_service::replica::DbPermissionRepo::new(policy.client()),
            management_service::replica::DbRoleRepo::new(policy.client()),
        );
        let (role_id, permission_id) = public_context_grant.expect("exact context grant");
        service
            .remove_permission_from_role(
                idp_model::contract::INSTALLATION_POLICY_ID,
                role_id,
                permission_id,
            )
            .await
            .expect("learn context permission removal");
        assert!(
            !runtime
                .node_public_ready()
                .await
                .expect("live current grant readiness"),
            "old completion cannot override learned grant denial"
        );
        service
            .add_permission_to_role(
                idp_model::contract::INSTALLATION_POLICY_ID,
                role_id,
                permission_id,
            )
            .await
            .expect("authorized grant restoration");
        assert!(
            runtime
                .node_public_ready()
                .await
                .expect("live restored grant readiness")
        );

        drop(service);
        drop(policy);
        let shared_endpoint = runtime.endpoint.server().endpoint().clone();
        cancellation_token.cancel();
        assert!(
            runtime.start_background_tasks().await.is_err(),
            "cancelled runtime must refuse background restart even after initial startup"
        );
        serve_task
            .await
            .expect("join unified server")
            .expect("serve and stop unified runtime");
        runtime.shutdown().await.expect("close shared runtime");
        assert!(
            protocol_router.is_shutdown(),
            "shutdown must join the router even with another handle alive"
        );
        assert!(
            shared_endpoint.is_closed(),
            "shared endpoint must be closed before shutdown returns"
        );
        let restored_endpoint =
            crate::UnifiedEndpoint::open(&root.join("endpoint"), &persisted_idp_config)
                .await
                .expect("reopen unified endpoint after shutdown");
        assert_eq!(restored_endpoint.endpoint_id(), endpoint_id);
        let reopened_engine = idp_server::open_local_identity_database(std::path::Path::new(
            &persisted_idp_config.data_dir,
        ))
        .await
        .expect("reopen persisted IdP without peer contact");
        let reopened_management = device_server::open_local_device_database(&root.join("device"))
            .await
            .expect("reopen Management offline");
        let devices = std::sync::Arc::new(device_service::replica::DbDeviceRepo::new(
            reopened_management.clone().client(),
        ));
        let restored_idp = idp_server::build_runtime(
            &persisted_idp_config,
            reopened_engine.client(),
            Some(devices.clone()),
            std::sync::Arc::new(restored_endpoint.identity().expect("restored identity")),
            restored_endpoint.server().clone(),
            None,
        )
        .await
        .expect("reopen local signing and trust runtime");
        let restored_control = super::build_control_plane(
            std::sync::Arc::new(restored_idp),
            devices,
            "http://127.0.0.1:1",
            "http://127.0.0.1:1",
            &issuer,
        )
        .expect("restored Management authority");
        restored_endpoint
            .refresh_approved_peers(&restored_control)
            .await
            .expect("restore persisted membership offline");
        assert!(
            restored_endpoint
                .server()
                .peers()
                .contains(approved_peer_id)
        );
        assert!(
            restored_endpoint
                .server()
                .peers()
                .contains(device_only_peer)
        );
        idp_service::replica::DbReplicaSignerRepo::new(reopened_engine.client())
            .revoke(peer_binding.member_id, &issuer, 2)
            .await
            .expect("learn signer-only revocation");
        restored_endpoint
            .refresh_approved_peers(&restored_control)
            .await
            .expect("refresh approval after signer revocation");
        assert!(
            restored_endpoint
                .server()
                .peers()
                .contains(approved_peer_id),
            "signer revocation leaves Device approval intact"
        );
        use device_service::DeviceRepo;
        assert!(
            device_service::replica::DbDeviceRepo::new(reopened_management.client())
                .revoke(
                    &fixture.administrator.to_string(),
                    peer_binding.member_id,
                    &endpoint_id.to_string(),
                )
                .await
                .expect("learn Device revocation")
        );
        restored_endpoint
            .refresh_approved_peers(&restored_control)
            .await
            .expect("apply learned Device admission removal");
        assert!(
            !restored_endpoint
                .server()
                .peers()
                .contains(approved_peer_id)
        );
        assert!(
            restored_endpoint
                .server()
                .peers()
                .contains(device_only_peer)
        );
        assert!(restored_endpoint.server().peers().contains(endpoint_id));
        restored_endpoint.close().await;
        drop(restored_control);
        fs::remove_dir_all(root).expect("remove unified runtime data");
    }

    async fn issue_client_token(
        client: &reqwest::Client,
        address: &std::net::SocketAddr,
        client_id: &str,
        client_secret: &str,
        audience: &str,
        scope: &str,
    ) -> String {
        let response = client
            .post(format!("http://{address}/idp/oauth2/token"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!(
                "grant_type=client_credentials&client_id={client_id}&client_secret={client_secret}&audience={audience}&scope={scope}"
            ))
            .send()
            .await
            .expect("request client-credentials token");
        assert_eq!(response.status(), http::StatusCode::OK);
        response
            .json::<serde_json::Value>()
            .await
            .expect("parse client-credentials response")["access_token"]
            .as_str()
            .expect("access token is present")
            .to_owned()
    }

    async fn get_json(client: &reqwest::Client, url: String) -> serde_json::Value {
        let response = client.get(url).send().await.expect("request JSON route");
        assert_eq!(response.status(), http::StatusCode::OK);
        let body = response.bytes().await.expect("read JSON response");
        serde_json::from_slice(&body).expect("parse JSON response")
    }
}

#[cfg(all(test, unix))]
#[path = "full_node_runtime_fixture.rs"]
mod full_node_fixture;

pub fn compose_router(idp: Router, management: Router, device: Router, storage: Router) -> Router {
    idp.merge(management).merge(device).merge(storage)
}

pub fn compose_protocol_router(
    server: &iroh_chain::Server,
    storage: &StorageRuntime,
    security: &SecurityProtocol,
) -> Option<IrohRouter> {
    storage.data_handler().map(|data_protocol| {
        let owner = security.clone();
        let data_protocol =
            data_protocol.with_additional_stream(move |kind, connection, send, recv| {
                let owner = owner.clone();
                async move { owner.accept_data_stream(kind, connection, send, recv).await }
            });
        server.router_with_protocol(
            data_protocol,
            idp_server::BOOTSTRAP_ALPN,
            security.node_bootstrap_handler(),
        )
    })
}
