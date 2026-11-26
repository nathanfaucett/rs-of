use std::{fmt, io, path::Path, sync::Arc, thread};

use axum::Router;
use db::adapters::EmbeddedDatabase;
use iroh::{EndpointId, SecretKey};
use iroh_chain::Server;
use management_service::service::MANAGEMENT_APPLICATION_URI;
use storage_service::{
    database_runtime::DatabaseRuntime, scoped_file_system::ScopedFileSystemRuntime,
};
use tokio::runtime::Builder;
use tokio_util::sync::CancellationToken;

#[cfg(feature = "sync")]
use crate::sync::{local_sync_router, sync_router};
use crate::{
    copy_retirement::StorageCopyRetirement,
    data_protocol::DataProtocolHandler,
    device_client::DeviceClient,
    idp_client::IdpClient,
    local_replication_authorizer::LocalReplicationAuthorizer,
    replication_runtime::StorageReplicationRuntime,
    router::{openapi_router::openapi_router, resources::resource_router, state::RouterState},
};

pub struct StorageRuntime {
    router: Router,
    replication: Option<StorageReplicationRuntime>,
    cancellation_token: CancellationToken,
    copies: Option<StorageCopyRetirement>,
}

impl fmt::Debug for StorageRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorageRuntime").finish_non_exhaustive()
    }
}

impl StorageRuntime {
    #[must_use]
    pub fn copy_retirement(&self) -> Option<StorageCopyRetirement> {
        self.copies.clone()
    }

    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn data_handler(&self) -> Option<DataProtocolHandler> {
        self.replication
            .as_ref()
            .map(StorageReplicationRuntime::data_handler)
    }

    /// Install merged IdP/Management changes before starting replication.
    pub fn set_policy_changes(&mut self, subscription: ::ofdb_sync::ChangeSubscription) {
        if let Some(replication) = self.replication.as_mut() {
            replication.set_policy_changes(subscription);
        }
    }

    /// Starts configured replication tasks.
    ///
    /// # Errors
    /// Returns an error if replication cannot start.
    pub fn start_background_tasks(&mut self) -> io::Result<()> {
        if let Some(replication) = self.replication.as_mut() {
            replication.start_sync(self.cancellation_token.clone())?;
        }
        Ok(())
    }

    /// Stops configured replication tasks.
    ///
    /// # Errors
    /// Returns an error if replication shutdown fails.
    pub async fn shutdown(self) -> io::Result<()> {
        self.cancellation_token.cancel();
        if let Some(replication) = self.replication {
            replication.shutdown().await?;
        }
        Ok(())
    }
}

/// Builds local Storage with all resource state inside the shared embedded host.
///
/// # Errors
/// Returns an error for mismatched injected state or owner initialization failure.
pub async fn build_local_runtime(
    data_dir: &Path,
    _port: u16,
    idp_base: &str,
    device_base: &str,
    db: Option<Arc<ofdb_sql::Database>>,
    shared_storage: Option<btree_redb::SharedDatabase>,
    http_client: Option<reqwest::Client>,
) -> io::Result<StorageRuntime> {
    let (client, storage) = match (db, shared_storage) {
        (Some(database), Some(storage)) => (database.client(), storage),
        (None, None) => {
            let database = db::local_database(data_dir, None).await?;
            (database.client(), database.storage())
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "shared SQL database and retained storage must be provided together",
            ));
        }
    };
    let peer = local_peer(client.clone())?;
    let idp = IdpClient::new(
        idp_base,
        "local-storage",
        "local-password",
        idp_base,
        idp_base,
    )
    .map_err(io::Error::other)?;
    let device = DeviceClient::new(
        device_base,
        idp_base,
        "local-storage",
        "local-password",
        idp_base,
        MANAGEMENT_APPLICATION_URI,
    )
    .map_err(io::Error::other)?;
    let (idp, device) = match http_client {
        Some(client) => (
            idp.with_http_client(client.clone()),
            device.with_http_client(client),
        ),
        None => (idp, device),
    };
    let state = RouterState::new(
        format!("{}/storage", idp_base.trim_end_matches("/idp")),
        peer.to_string(),
    )
    .with_idp_client(idp)
    .with_device_client(device);
    build_runtime_inner(
        state,
        data_dir,
        "/storage",
        "",
        None,
        Some(peer),
        CancellationToken::new(),
        Some((&storage, client)),
        true,
    )
}

fn local_peer(client: ofdb_sql::Client) -> io::Result<EndpointId> {
    thread::spawn(move || {
        Builder::new_current_thread().build()?.block_on(async {
            client.execute_sql("CREATE TABLE IF NOT EXISTS local_storage_peer (id UUID PRIMARY KEY, public_key TEXT NOT NULL)", None).await.map_err(io::Error::other)?;
            let rows = client.execute_sql("SELECT public_key FROM local_storage_peer", None).await.map_err(io::Error::other)?;
            match rows[0].rows.as_slice() {
                [] => {
                    let peer = SecretKey::generate().public();
                    client.execute_sql(&format!("INSERT INTO local_storage_peer (public_key) VALUES ('{peer}')"), None).await.map_err(io::Error::other)?;
                    Ok(peer)
                }
                [row] => {
                    let value: String = ofdb_sql::decode(&row.values[0], "public_key").map_err(|error| io::Error::other(error.to_string()))?;
                    value.parse().map_err(io::Error::other)
                }
                _ => Err(io::Error::new(io::ErrorKind::InvalidData, "multiple local Storage peers")),
            }
        })
    }).join().map_err(|_error| io::Error::other("local Storage identity worker panicked"))?
}

/// Builds Storage with an optional network endpoint.
///
/// # Errors
/// Returns an error if resource runtimes cannot open retained state.
pub fn build_runtime(
    state: RouterState,
    data_dir: &Path,
    api_prefix: &str,
    resource_prefix: &str,
    server: Option<Server>,
    cancellation_token: CancellationToken,
) -> io::Result<StorageRuntime> {
    let endpoint = server.as_ref().map(|server| server.endpoint().id());
    build_runtime_inner(
        state,
        data_dir,
        api_prefix,
        resource_prefix,
        server,
        endpoint,
        cancellation_token,
        None,
        false,
    )
}

/// HTTPS Storage consumes the Management endpoint's current policy without owning its key.
///
/// # Errors
/// Returns an error if resource runtimes cannot open retained state.
pub fn build_http_runtime(
    state: RouterState,
    data_dir: &Path,
    api_prefix: &str,
    management_endpoint: EndpointId,
    cancellation_token: CancellationToken,
) -> io::Result<StorageRuntime> {
    build_runtime_inner(
        state,
        data_dir,
        api_prefix,
        "",
        None,
        Some(management_endpoint),
        cancellation_token,
        None,
        false,
    )
}

/// Builds HTTPS Storage on a shared embedded database with the supplied IdP and Device clients.
///
/// # Errors
/// Returns an error if resource runtimes cannot open retained state.
pub fn build_shared_http_runtime(
    state: RouterState,
    data_dir: &Path,
    api_prefix: &str,
    management_endpoint: EndpointId,
    database: &EmbeddedDatabase,
    cancellation_token: CancellationToken,
) -> io::Result<StorageRuntime> {
    build_runtime_inner(
        state,
        data_dir,
        api_prefix,
        "",
        None,
        Some(management_endpoint),
        cancellation_token,
        Some((&database.storage, database.client())),
        false,
    )
}

fn build_runtime_inner(
    mut state: RouterState,
    data_dir: &Path,
    api_prefix: &str,
    resource_prefix: &str,
    server: Option<Server>,
    endpoint: Option<EndpointId>,
    cancellation_token: CancellationToken,
    shared: Option<(&btree_redb::SharedDatabase, ofdb_sql::Client)>,
    seeded_fixture: bool,
) -> io::Result<StorageRuntime> {
    let (databases, shared_storage) = match shared {
        Some((storage, client)) => (
            DatabaseRuntime::with_shared(storage.clone(), client)?,
            Some(storage),
        ),
        None => (DatabaseRuntime::new(data_dir.join("databases"))?, None),
    };
    let databases = Arc::new(databases);
    let file_systems = endpoint
        .map(|endpoint| {
            match shared_storage {
                Some(storage) => ScopedFileSystemRuntime::with_shared(storage.clone(), endpoint),
                None => ScopedFileSystemRuntime::new(data_dir.join("filesystems"), endpoint),
            }
            .map(Arc::new)
        })
        .transpose()
        .map_err(io::Error::other)?;

    let copies = file_systems
        .as_ref()
        .map(|file_systems| StorageCopyRetirement {
            databases: Arc::clone(&databases),
            file_systems: Arc::clone(file_systems),
        });
    if !seeded_fixture && state.local_replication_authorizer.is_none() {
        state.local_replication_authorizer =
            state
                .device_client
                .clone()
                .map(|client| match (endpoint, copies.clone()) {
                    (Some(endpoint), Some(copies)) => {
                        LocalReplicationAuthorizer::with_device_cleanup(
                            client,
                            endpoint,
                            copies,
                            cancellation_token.clone(),
                        )
                    }
                    _ => LocalReplicationAuthorizer::from_device_client(client),
                });
    }
    if let (Some(endpoint), Some(policy), Some(idp)) = (
        endpoint,
        state.local_replication_authorizer.as_ref(),
        state.idp_client.clone(),
    ) {
        state.idp_client = Some(idp.with_local_device_policy(endpoint, policy.clone()));
    }

    let replication = match (
        server,
        state.local_replication_authorizer.clone(),
        file_systems.clone(),
    ) {
        (Some(server), Some(authorizer), Some(file_systems)) => {
            Some(StorageReplicationRuntime::start(
                server,
                authorizer,
                Arc::clone(&databases),
                file_systems,
                data_dir.join("sync-staging"),
                cancellation_token.clone(),
            ))
        }
        _ => None,
    };

    let resource_routes =
        resource_router(state.clone(), Some(Arc::clone(&databases)), file_systems);
    let resource_routes = if resource_prefix.is_empty() {
        resource_routes
    } else {
        Router::new().nest(resource_prefix, resource_routes)
    };
    #[cfg(feature = "sync")]
    let router_state = state.clone();
    #[cfg(not(feature = "sync"))]
    let router_state = state;
    let router = openapi_router(router_state, api_prefix)
        .split_for_parts()
        .0
        .merge(resource_routes);

    #[cfg(feature = "sync")]
    let router = router.merge(if seeded_fixture {
        local_sync_router(state, Arc::clone(&databases), data_dir.join("sync-staging"))
    } else {
        sync_router(state, Arc::clone(&databases), data_dir.join("sync-staging"))
    });

    Ok(StorageRuntime {
        router,
        replication,
        cancellation_token,
        copies,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use crate::router::state::RouterState;
    use crate::runtime::build_runtime;

    #[tokio::test]
    async fn remote_policy_enables_replication_without_replacing_local_policy() {
        let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
            .bind()
            .await
            .expect("bind isolated Storage endpoint");
        let server = iroh_chain::Server::new(endpoint, iroh_chain::EndpointIdStore::default());
        let data_dir = std::env::temp_dir().join(format!(
            "storage-local-policy-{}",
            of_model::id::Id::now_v7()
        ));
        let cancellation = tokio_util::sync::CancellationToken::new();
        let remote = crate::device_client::DeviceClient::new(
            "https://device.example",
            "https://idp.example",
            "storage-client",
            "storage-secret",
            "https://idp.example",
            "management-service",
        )
        .expect("configure authenticated remote policy source");
        let remote_policy = build_runtime(
            RouterState::new(
                "https://storage.example",
                server.endpoint().id().to_string(),
            )
            .with_device_client(remote.clone()),
            &data_dir,
            "/storage",
            "",
            Some(server.clone()),
            cancellation.clone(),
        )
        .expect("runtime without local policy");
        assert!(remote_policy.data_handler().is_some());
        remote_policy
            .shutdown()
            .await
            .expect("stop remote-policy runtime");

        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let local_calls = calls.clone();
        let policy =
            crate::local_replication_authorizer::LocalReplicationAuthorizer::new(move |_| {
                local_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async { Err("local policy unavailable".to_owned()) }
            });
        let idp = crate::idp_client::IdpClient::local(|_| async {
            Ok(
                of_model::idp::contract::introspection_response::IntrospectionResponse {
                    claims: of_model::contract::standard_claims::StandardClaims {
                        r#type: of_model::contract::token_type::TokenType::Bearer,
                        r#use: of_model::contract::token_use::TokenUse::Access,
                        exp: i64::MAX,
                        iat: 0,
                        nbf: 0,
                        iss: "https://idp.example".into(),
                        aud: "https://storage.example".into(),
                        client_id: "application-client".into(),
                        sub: of_model::id::Id::now_v7().to_string(),
                        principal_type: of_model::contract::principal_type::PrincipalType::User,
                        resource: None,
                        authorization_details: None,
                        scope: vec![],
                    },
                    application_id: of_model::id::Id::now_v7().to_string(),
                },
            )
        });
        let runtime = build_runtime(
            RouterState::new(
                "https://storage.example",
                server.endpoint().id().to_string(),
            )
            .with_device_client(remote)
            .with_idp_client(idp)
            .with_local_replication_authorizer(policy.clone()),
            &data_dir,
            "/storage",
            "",
            Some(server.clone()),
            cancellation.clone(),
        )
        .expect("runtime needs local policy, not remote credentials");
        assert!(runtime.data_handler().is_some());
        let response = runtime
            .router()
            .oneshot(
                Request::builder()
                    .uri("/storage/databases")
                    .header(axum::http::header::AUTHORIZATION, "Bearer user-token")
                    .body(Body::empty())
                    .expect("request with current local policy"),
            )
            .await
            .expect("local policy check");
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        runtime.shutdown().await.expect("stop local-policy runtime");
        let local_only = build_runtime(
            RouterState::new(
                "https://storage.example",
                server.endpoint().id().to_string(),
            )
            .with_local_replication_authorizer(policy),
            &data_dir,
            "/storage",
            "",
            Some(server.clone()),
            cancellation.clone(),
        )
        .expect("local policy requires no remote credentials");
        assert!(local_only.data_handler().is_some());
        cancellation.cancel();
        local_only
            .shutdown()
            .await
            .expect("stop offline-first runtime");
        server.endpoint().close().await;
        fs::remove_dir_all(data_dir).expect("remove isolated Storage data");
    }

    #[tokio::test]
    async fn unified_prefix_does_not_repeat_storage_path() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after Unix epoch")
            .as_nanos();
        let data_dir =
            std::env::temp_dir().join(format!("storage-runtime-{}-{unique}", std::process::id()));
        let runtime = build_runtime(
            RouterState::new(
                "http://storage.local",
                iroh::SecretKey::generate().public().to_string(),
            ),
            &data_dir,
            "/storage",
            "",
            None,
            tokio_util::sync::CancellationToken::new(),
        )
        .expect("build Storage runtime");

        let router = runtime.router();
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/storage/databases")
                    .header(axum::http::header::AUTHORIZATION, "Bearer token")
                    .body(Body::empty())
                    .expect("build storage route request"),
            )
            .await
            .expect("call storage route");
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/storage/storage/databases")
                    .body(Body::empty())
                    .expect("build duplicate-prefix request"),
            )
            .await
            .expect("call duplicate-prefix route");
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);

        runtime.shutdown().await.expect("stop Storage runtime");
        fs::remove_dir_all(data_dir).expect("remove temporary Storage data");
    }
}
