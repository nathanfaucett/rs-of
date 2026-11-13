use alloc::vec::Vec;

use of_model::idp::model::{Id, Permission};

use crate::{ManagementResult, SyncSelection};

pub trait PermissionRepo {
    fn list_permissions(
        &self,
        application_id: Id,
        offset: u32,
        limit: u32,
    ) -> impl Future<Output = ManagementResult<Vec<Permission>>>;

    fn create_permission(
        &self,
        application_id: Id,
        name: &str,
        description: Option<&str>,
    ) -> impl Future<Output = ManagementResult<Permission>>;

    fn create_permission_with_id(
        &self,
        id: Id,
        application_id: Id,
        name: &str,
        description: Option<&str>,
    ) -> impl Future<Output = ManagementResult<Permission>>;

    fn find_permission_by_id(
        &self,
        application_id: Id,
        permission_id: Id,
    ) -> impl Future<Output = ManagementResult<Option<Permission>>>;

    fn delete_permission_by_id(
        &self,
        application_id: Id,
        permission_id: Id,
    ) -> impl Future<Output = ManagementResult<()>>;

    fn add_permission_to_role(
        &self,
        application_id: Id,
        role_id: Id,
        permission_id: Id,
    ) -> impl Future<Output = ManagementResult<()>>;

    fn remove_permission_from_role(
        &self,
        application_id: Id,
        role_id: Id,
        permission_id: Id,
    ) -> impl Future<Output = ManagementResult<()>>;

    fn list_role_permissions(
        &self,
        application_id: Id,
        role_id: Id,
    ) -> impl Future<Output = ManagementResult<Vec<Permission>>>;

    // Sync selection methods
    fn select_resource(
        &self,
        device_id: Id,
        resource_id: Id,
        application_id: Id,
        owner_subject: &str,
    ) -> impl Future<Output = ManagementResult<()>>;

    fn deselect_resource(
        &self,
        device_id: Id,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<()>>;

    fn list_device_selections(
        &self,
        device_id: Id,
    ) -> impl Future<Output = ManagementResult<Vec<SyncSelection>>>;

    fn list_resource_sync_targets(
        &self,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<Vec<SyncSelection>>>;

    fn has_selection(
        &self,
        device_id: Id,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<bool>>;

    fn clear_device_selections(
        &self,
        device_id: Id,
    ) -> impl Future<Output = ManagementResult<()>>;
}
