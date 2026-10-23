use crate::{RouterState, router::openapi_router};
use axum::Router;
use db::Database;
use device_service::HostedControlPlane;
use management_service::{
    ManagementService,
    replica::{DbPermissionRepo, DbRoleRepo},
};
use std::sync::Arc;
pub async fn build_router(
    identity: Arc<Database>,
    api_base_uri: &str,
    prefix: &str,
    control_plane: Arc<HostedControlPlane>,
) -> Result<Router, db::QueryError> {
    let service = Arc::new(ManagementService::new(
        DbPermissionRepo::new(Arc::clone(&identity)),
        DbRoleRepo::new(identity),
    ));
    Ok(openapi_router(
        RouterState::new(api_base_uri, service, control_plane),
        prefix,
    )
    .into())
}
