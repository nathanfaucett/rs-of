use std::{io, path::Path, sync::Arc};

use axum::Router;
use iroh::EndpointId;
use iroh_chain::Server;
use storage_service::{DatabaseRuntime, ScopedFileSystemRuntime};
use tokio_util::sync::CancellationToken;

use crate::{RouterState, StorageReplicationRuntime, openapi_router, resource_router};

pub struct StorageRuntime {
    router: Router,
    replication: Option<StorageReplicationRuntime>,
    cancellation_token: CancellationToken,
    copies: Option<crate::StorageCopyRetirement>,
}

impl StorageRuntime {
    pub fn copy_retirement(&self) -> Option<crate::StorageCopyRetirement> {
        self.copies.clone()
    }

    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn data_handler(&self) -> Option<crate::DataProtocolHandler> {
        self.replication
            .as_ref()
            .map(StorageReplicationRuntime::data_handler)
    }

    /// Install merged IdP/Management changes before starting replication.
    pub fn set_policy_changes(&mut self, subscription: ofdb_sql::ChangeSubscription) {
        if let Some(replication) = self.replication.as_mut() {
            replication.set_policy_changes(subscription);
        }
    }

    pub fn start_background_tasks(&mut self) -> io::Result<()> {
        if let Some(replication) = self.replication.as_mut() {
            replication.start_sync(self.cancellation_token.clone())?;
        }
        Ok(())
    }

    pub async fn shutdown(self) -> io::Result<()> {
        self.cancellation_token.cancel();
        if let Some(replication) = self.replication {
            replication.shutdown().await?;
        }
        Ok(())
    }
}

/// Builds local Storage with a shared SQL catalog and file-backed resource contents.
pub fn build_local_runtime(
    data_dir: &Path,
    port: u16,
    idp_base: &str,
    device_base: &str,
    db: Option<Arc<dyn of_model::db::Database>>,
) -> io::Result<StorageRuntime> {
    let client = embedded_db::local_client(data_dir, "storage", db)?;
    std::fs::create_dir_all(data_dir)?;
    let peer_path = data_dir.join("storage-local.peer");
    let peer = match std::fs::read_to_string(&peer_path) {
        Ok(peer) => peer.parse::<EndpointId>().map_err(io::Error::other)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            use std::io::Write;
            let peer = iroh::SecretKey::generate().public();
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&peer_path)?;
            file.write_all(peer.to_string().as_bytes())?;
            file.sync_all()?;
            peer
        }
        Err(error) => return Err(error),
    };
    let idp = crate::IdpClient::new(
        idp_base,
        "local-storage",
        "local-password",
        idp_base,
        idp_base,
    )
    .map_err(io::Error::other)?;
    let device = crate::DeviceClient::new(
        device_base,
        idp_base,
        "local-storage",
        "local-password",
        idp_base,
        management_service::MANAGEMENT_APPLICATION_URI,
    )
    .map_err(io::Error::other)?;
    let state = RouterState::new(format!("http://127.0.0.1:{port}/storage"), peer.to_string())
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
        Some(client),
    )
}

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
    )
}

/// HTTPS Storage consumes the Management endpoint's current policy without owning its key.
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
    catalog_client: Option<db::Client>,
) -> io::Result<StorageRuntime> {
    let local = catalog_client.is_some();
    let databases = Arc::new(match catalog_client {
        Some(client) => DatabaseRuntime::with_client(data_dir.join("databases"), client)?,
        None => DatabaseRuntime::new(data_dir.join("databases"))?,
    });
    let file_systems = endpoint
        .map(|endpoint| {
            ScopedFileSystemRuntime::<EndpointId>::new(data_dir.join("filesystems"), endpoint)
                .map(Arc::new)
        })
        .transpose()
        .map_err(io::Error::other)?;

    let copies = file_systems
        .as_ref()
        .map(|file_systems| crate::StorageCopyRetirement {
            databases: Arc::clone(&databases),
            file_systems: Arc::clone(file_systems),
        });
    if !local && state.local_replication_authorizer.is_none() {
        state.local_replication_authorizer =
            state
                .device_client
                .clone()
                .map(|client| match (endpoint, copies.clone()) {
                    (Some(endpoint), Some(copies)) => {
                        crate::LocalReplicationAuthorizer::with_device_cleanup(
                            client,
                            endpoint,
                            copies,
                            cancellation_token.clone(),
                        )
                    }
                    _ => crate::LocalReplicationAuthorizer::from_device_client(client),
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

    let resource_routes = resource_router(
        state.clone(),
        Some(Arc::clone(&databases)),
        file_systems.clone(),
    );
    let resource_routes = if resource_prefix.is_empty() {
        resource_routes
    } else {
        Router::new().nest(resource_prefix, resource_routes)
    };
    let router = openapi_router(state.clone(), api_prefix)
        .split_for_parts()
        .0
        .merge(resource_routes);

    #[cfg(feature = "sync")]
    let router = router.merge(crate::sync::sync_router(
        state,
        Arc::clone(&databases),
        data_dir.join("sync-staging"),
    ));

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

    use super::build_runtime;
    use crate::RouterState;

    #[tokio::test]
    async fn remote_policy_enables_replication_without_replacing_local_policy() {
        let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
            .bind()
            .await
            .expect("bind isolated Storage endpoint");
        let server = iroh_chain::Server::new(endpoint, iroh_chain::EndpointIdStore::default());
        let data_dir = std::env::temp_dir().join(format!(
            "storage-local-policy-{}",
            of_model::idp::model::Id::now_v7()
        ));
        let cancellation = tokio_util::sync::CancellationToken::new();
        let remote = crate::DeviceClient::new(
            "https://device.example",
            "https://idp.example",
            "storage-client",
            "storage-secret",
            "https://idp.example",
            "management-service",
        )
        .expect("configure authenticated remote policy source");
        let remote_policy = build_runtime(
            RouterState::new("https://storage.example").with_device_client(remote.clone()),
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
        let policy = crate::LocalReplicationAuthorizer::new(move |_| {
            local_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async { Err("local policy unavailable".to_owned()) }
        });
        let idp = crate::IdpClient::local(|_| async {
            Ok(of_model::idp::contract::IntrospectionResponse {
                claims: of_model::contract::StandardClaims {
                    r#type: of_model::contract::TokenType::Bearer,
                    r#use: of_model::contract::TokenUse::Access,
                    exp: i64::MAX,
                    iat: 0,
                    nbf: 0,
                    iss: "https://idp.example".into(),
                    aud: "https://storage.example".into(),
                    client_id: "application-client".into(),
                    sub: of_model::idp::model::Id::now_v7().to_string(),
                    principal_type: of_model::contract::PrincipalType::User,
                    resource: None,
                    authorization_details: None,
                    scope: vec![],
                },
                application_id: of_model::idp::model::Id::now_v7().to_string(),
            })
        });
        let runtime = build_runtime(
            RouterState::new("https://storage.example")
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
            RouterState::new("https://storage.example").with_local_replication_authorizer(policy),
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
            RouterState::new("http://storage.local"),
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
