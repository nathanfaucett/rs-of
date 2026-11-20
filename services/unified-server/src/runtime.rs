use std::{io, sync::Arc, time::Duration};

use axum::{
    Router,
    extract::Request,
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
};
use db::adapters::EmbeddedDatabase;
use reqwest::{Client, Url, redirect::Policy};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use tower_http::cors::{Any, CorsLayer};

use crate::config::UnifiedConfig;

/// Routes each request to its owner prefix.
///
/// # Panics
/// Panics if an owner router violates Axum's infallible response contract.
pub fn compose_router(idp: Router, management: Router, device: Router, storage: Router) -> Router {
    Router::new().fallback(move |request: Request| {
        let router = match request.uri().path().split('/').nth(1) {
            Some("idp") => Some(idp.clone()),
            Some("management") => Some(management.clone()),
            Some("device") => Some(device.clone()),
            Some("storage") => Some(storage.clone()),
            _ => None,
        };
        async move {
            match router {
                Some(router) => router
                    .oneshot(request)
                    .await
                    .expect("infallible owner router"),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    })
}

fn base_url(config: &UnifiedConfig) -> String {
    config
        .public_base_url
        .clone()
        .unwrap_or_else(|| format!("http://{}", config.listen_addr))
        .trim_end_matches('/')
        .to_owned()
}

fn frontend_origin(value: &str) -> io::Result<Url> {
    let origin = Url::parse(value).map_err(io::Error::other)?;
    if !matches!(origin.scheme(), "http" | "https")
        || origin.host_str().is_none()
        || !origin.username().is_empty()
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
        || origin.path() != "/"
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "origin must be an HTTP origin without a path, credentials, query, or fragment",
        ));
    }
    Ok(origin)
}

/// Builds the four-owner local router over one retained database.
///
/// # Errors
/// Returns an error for invalid configuration, database failure, or owner initialization failure.
pub async fn compose_local_router(config: &UnifiedConfig) -> io::Result<Router> {
    if !config.listen_addr.ip().is_loopback() || config.listen_addr.port() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local mode requires a fixed loopback address",
        ));
    }
    let origin = frontend_origin(&config.origin)?;
    let password_manager = if let Some(origin) = config.password_manager_origin.as_deref() {
        frontend_origin(origin)?
    } else {
        let mut password_manager = origin.clone();
        password_manager
            .set_port(Some(5174))
            .map_err(|()| io::Error::other("invalid frontend port"))?;
        password_manager
    };
    let http_client = Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(io::Error::other)?;
    let shared_db = Arc::new(
        EmbeddedDatabase::open(&config.data_dir)
            .await
            .map_err(io::Error::other)?,
    );
    let shared_database = shared_db.database.clone();
    let base = base_url(config);
    let idp_base = format!("{base}/idp");
    let device_base = format!("{base}/device");
    let storage_base = format!("{base}/storage");
    let port = config.listen_addr.port();
    let idp = idp_server::build_local_router(
        &config.data_dir,
        &idp_base,
        &config.origin,
        config.password_manager_origin.as_deref(),
        Some(Arc::new(shared_database.clone())),
        Some(shared_db.storage.clone()),
        &config.seed.as_ref().map_or_default(|seed| {
            seed.users
                .iter()
                .map(|user| (user.username.clone(), user.password.clone()))
                .collect::<Vec<_>>()
        }),
    )
    .await?;
    let device = device_server::build_local_router(
        &config.data_dir.join("device"),
        port,
        &idp_base,
        Some(Arc::new(shared_db.database.clone())),
        Some(http_client.clone()),
    )
    .await?;
    let management = management_server::build_local_router(
        &config.data_dir.join("management"),
        port,
        &idp_base,
        &storage_base,
        Some(Arc::new(shared_db.database.clone())),
        Some(http_client.clone()),
    )
    .await?;
    let storage = storage_server::build_local_runtime(
        &config.data_dir.join("storage"),
        port,
        &idp_base,
        &device_base,
        Some(Arc::new(shared_db.database.clone())),
        Some(shared_db.storage.clone()),
        Some(http_client),
    )
    .await?;
    let app_origin: HeaderValue = origin
        .origin()
        .ascii_serialization()
        .parse()
        .map_err(io::Error::other)?;
    let password_manager_origin: HeaderValue = password_manager
        .origin()
        .ascii_serialization()
        .parse()
        .map_err(io::Error::other)?;
    let cors = CorsLayer::new()
        .allow_origin(vec![app_origin, password_manager_origin])
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_private_network(true);
    Ok(compose_router(idp, management, device, storage.router()).layer(cors))
}

/// Serves local composition until cancellation.
///
/// # Errors
/// Returns an error if the listener, composition, or HTTP server fails.
pub async fn serve(config: UnifiedConfig, cancellation: CancellationToken) -> io::Result<()> {
    let listener = TcpListener::bind(config.listen_addr).await?;
    let router = compose_local_router(&config).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(cancellation.cancelled_owned())
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_base_uses_public_url_when_proxy_supplies_one() {
        let mut config = UnifiedConfig {
            listen_addr: "127.0.0.1:4000".parse().expect("loopback address"),
            public_base_url: Some("https://unified.localhost:1355/".into()),
            data_dir: ".data".into(),
            origin: "http://localhost:5173".into(),
            password_manager_origin: None,
            seed: None,
        };
        assert_eq!(base_url(&config), "https://unified.localhost:1355");

        config.public_base_url = None;
        assert_eq!(base_url(&config), "http://127.0.0.1:4000");
    }
}
