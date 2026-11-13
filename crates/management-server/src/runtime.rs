use crate::{RouterState, router::openapi_router};
use axum::Router;
use db::Client;
use device_service::HostedControlPlane;
use management_service::{
    ManagementService,
    replica::{DbPermissionRepo, DbRoleRepo},
};
use std::sync::Arc;
/// Builds the local Management owner without setup or admission.
pub async fn build_local_router(
    data_dir: &std::path::Path,
    port: u16,
    idp_base: &str,
    storage_base: &str,
    db: Option<Arc<dyn of_model::db::Database>>,
) -> std::io::Result<Router> {
    let client = embedded_db::local_client(data_dir, "management", db)?;
    let control = HostedControlPlane::new_with_services(idp_base, storage_base, idp_base)
        .and_then(|control| {
            control.with_idp_service_client("local-management", "local-password", idp_base)
        })
        .map_err(std::io::Error::other)?;
    build_router(
        client,
        &format!("http://127.0.0.1:{port}/management"),
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
) -> Result<Router, db::QueryError> {
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
