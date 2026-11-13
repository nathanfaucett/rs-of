use std::{io, sync::Arc};

use axum::{Router, extract::Request, http::StatusCode, response::IntoResponse};
use embedded_db::EmbeddedDatabase;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use tower_http::cors::{Any, CorsLayer};

use crate::{UnifiedConfig, tls};

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

pub async fn compose_local_router(config: &UnifiedConfig) -> io::Result<Router> {
    if !config.listen_addr.ip().is_loopback() || config.listen_addr.port() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local mode requires a fixed loopback address",
        ));
    }
    let origin = reqwest::Url::parse(&config.origin).map_err(io::Error::other)?;
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
    tls::load(config)?;
    let http_client = tls::http_client(config)?;
    let db = Arc::new(
        EmbeddedDatabase::open(&config.data_dir)
            .await
            .map_err(io::Error::other)?,
    );
    let scheme = if matches!(config.tls, crate::TlsConfig::Disabled) {
        "http"
    } else {
        "https"
    };
    let base = format!("{scheme}://{}", config.listen_addr);
    let idp_base = format!("{base}/idp");
    let device_base = format!("{base}/device");
    let storage_base = format!("{base}/storage");
    let port = config.listen_addr.port();
    let idp = idp_server::build_local_router(
        &config.data_dir,
        &idp_base,
        &config.origin,
        Some(db.clone()),
        &config
            .seed
            .as_ref()
            .map(|seed| {
                seed.users
                    .iter()
                    .map(|user| (user.username.clone(), user.password.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    )
    .await?;
    let device = device_server::build_local_router(
        &config.data_dir.join("device"),
        port,
        &idp_base,
        Some(db.clone()),
        Some(http_client.clone()),
    )
    .await?;
    let management = management_server::build_local_router(
        &config.data_dir.join("management"),
        port,
        &idp_base,
        &storage_base,
        Some(db.clone()),
        Some(http_client.clone()),
    )
    .await?;
    let storage = storage_server::build_local_runtime(
        &config.data_dir.join("storage"),
        port,
        &idp_base,
        &device_base,
        Some(db),
        Some(http_client),
    )?;
    let app_origin: axum::http::HeaderValue = origin
        .origin()
        .ascii_serialization()
        .parse()
        .map_err(io::Error::other)?;
    let mut password_manager = origin;
    password_manager
        .set_port(Some(5174))
        .map_err(|_| io::Error::other("invalid frontend port"))?;
    let password_manager_origin: axum::http::HeaderValue = password_manager
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

pub async fn serve(config: UnifiedConfig, cancellation: CancellationToken) -> io::Result<()> {
    let listener = tokio::net::TcpListener::bind(config.listen_addr).await?;
    let router = compose_local_router(&config).await?;
    let tls = tls::load(&config)?;
    match tls {
        Some(tls) => {
            axum::serve(tls::TlsListener::new(listener, tls), router)
                .with_graceful_shutdown(cancellation.cancelled_owned())
                .await
        }
        None => {
            axum::serve(listener, router)
                .with_graceful_shutdown(cancellation.cancelled_owned())
                .await
        }
    }
}
