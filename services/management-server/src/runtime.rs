use crate::router::{openapi_router::openapi_router, state::RouterState};
use axum::Router;
use db::local_client;
use device_service::hosted_control_plane::HostedControlPlane;
use management_service::ofdb::{DbPermissionRepo, DbRoleRepo};
use management_service::service::ManagementService;
use ofdb_sql::Client;
use ofdb_sql::Database as SqlDatabase;
use std::sync::Arc;
/// Builds the local Management owner without setup or admission.
pub async fn build_local_router(
    data_dir: &std::path::Path,
    _port: u16,
    idp_base: &str,
    storage_base: &str,
    db: Option<Arc<SqlDatabase>>,
    http_client: Option<reqwest::Client>,
) -> std::io::Result<Router> {
    let client = local_client(data_dir, db).await?;
    let control = HostedControlPlane::new_with_services(idp_base, storage_base, idp_base)
        .and_then(|control| {
            control.with_idp_service_client("local-management", "local-password", idp_base)
        })
        .map_err(std::io::Error::other)?;
    let control = match http_client {
        Some(client) => control.with_http_client(client),
        None => control,
    };
    build_router(
        client,
        &format!("{}/management", idp_base.trim_end_matches("/idp")),
        "/management",
        Arc::new(control),
    )
    .await
    .map_err(std::io::Error::other)
}

pub async fn build_router(
    identity: Client,
    api_base_uri: &str,
    prefix: &str,
    control_plane: Arc<HostedControlPlane>,
) -> Result<Router, ofdb_sql::QueryError> {
    let service = Arc::new(ManagementService::new(
        DbPermissionRepo::new(identity.clone()),
        DbRoleRepo::new(identity),
    ));
    Ok(openapi_router(
        RouterState::new(api_base_uri, service, control_plane),
        prefix,
    )
    .into())
}
