use crate::management_error::ManagementResult;
use crate::{idp::model::Id, management::SyncSelection};
use alloc::vec::Vec;

/// Sync selection repository trait.
pub trait SyncSelectionRepo {
    /// Select a resource for WebSocket sync on a device.
    fn select_resource(
        &self,
        device_id: Id,
        resource_id: Id,
        application_id: Id,
        owner_subject: &str,
    ) -> impl Future<Output = ManagementResult<()>>;

    /// Deselect a resource for WebSocket sync on a device.
    fn deselect_resource(
        &self,
        device_id: Id,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<()>>;

    /// List all selected resources for a device.
    fn list_device_selections(
        &self,
        device_id: Id,
    ) -> impl Future<Output = ManagementResult<Vec<SyncSelection>>>;

    /// List all devices that have a resource selected for WebSocket sync.
    fn list_resource_sync_targets(
        &self,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<Vec<SyncSelection>>>;

    /// Check if a device has a resource selected for WebSocket sync.
    fn has_selection(
        &self,
        device_id: Id,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<bool>>;

    /// Remove all selections for a device (when device is revoked).
    fn clear_device_selections(&self, device_id: Id) -> impl Future<Output = ManagementResult<()>>;
}

/// Management Service extension for WebSocket sync selection.
pub trait ManagementSyncSelection {
    /// Select a resource for WebSocket sync on a device.
    fn select_resource_for_sync(
        &self,
        device_id: Id,
        resource_id: Id,
        application_id: Id,
        owner_subject: &str,
    ) -> impl Future<Output = ManagementResult<()>> + Send;

    /// Deselect a resource for WebSocket sync on a device.
    fn deselect_resource_for_sync(
        &self,
        device_id: Id,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<()>> + Send;

    /// List selected resources for WebSocket sync on a device.
    fn list_sync_selections_for_device(
        &self,
        device_id: Id,
    ) -> impl Future<Output = ManagementResult<Vec<SyncSelection>>> + Send;

    /// List devices that have a resource selected for WebSocket sync.
    fn list_sync_targets_for_resource(
        &self,
        resource_id: Id,
    ) -> impl Future<Output = ManagementResult<Vec<SyncSelection>>> + Send;

    /// Get the sync target device for a resource from a source device.
    /// Returns the first device (other than source) that has the resource selected.
    fn get_sync_target(
        &self,
        resource_id: Id,
        source_device_id: Id,
    ) -> impl Future<Output = ManagementResult<Option<SyncSelection>>> + Send;
}
