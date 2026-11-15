use std::sync::Arc;

use device_service::hosted_control_plane::HostedControlPlane;
use management_service::replica::permission_repo::DbPermissionRepo;
use management_service::replica::role_repo::DbRoleRepo;
use management_service::service::ManagementService;

type DbPermission = DbPermissionRepo;
type DbRole = DbRoleRepo;

pub(crate) type ManagementRouterService = ManagementService<DbPermission, DbRole>;

#[derive(Clone)]
pub struct RouterState {
    pub(crate) api_base_uri: String,
    pub(crate) management_service: Arc<ManagementRouterService>,

    pub(crate) control_plane: Arc<HostedControlPlane>,
}

impl RouterState {
    pub fn new(
        api_base_uri: impl Into<String>,
        management_service: Arc<ManagementRouterService>,

        control_plane: Arc<HostedControlPlane>,
    ) -> Self {
        Self {
            api_base_uri: api_base_uri.into(),
            management_service,

            control_plane,
        }
    }
}
