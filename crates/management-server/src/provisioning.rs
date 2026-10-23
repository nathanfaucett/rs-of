use db::Database;
use idp_model::{
    contract::IdentityAction,
    model::{Id, Role},
};
use management_service::{
    ManagementService,
    replica::{DbPermissionRepo, DbRoleRepo},
};
use std::{io, sync::Arc};
pub async fn provision_initial_administrator(
    engine: Arc<Database>,
    user_id: Id,
    role_id: Id,
    permissions: &[(Id, IdentityAction)],
) -> io::Result<Role> {
    let service = ManagementService::new(
        DbPermissionRepo::new(Arc::clone(&engine)),
        DbRoleRepo::new(engine),
    );
    service
        .provision_initial_administrator(user_id, role_id, permissions)
        .await
        .map_err(io::Error::other)
}
