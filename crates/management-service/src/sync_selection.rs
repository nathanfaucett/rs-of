//! WebSocket sync selection for Management Service.
//!
//! This module provides the API for managing which devices have WebSocket sync
//! enabled for specific resources. This is parallel to the Iroh mesh selection
//! but for WebSocket-based sync (browser/server apps).

use alloc::{string::String, vec::Vec};
use core::fmt::Debug;

use idp_model::model::Id;
use serde::{Deserialize, Serialize};

use crate::ManagementResult;

/// Sync selection record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncSelection {
    pub device_id: Id,
    pub resource_id: Id,
    pub application_id: Id,
    pub owner_subject: String,
    pub selected_at: i64,
}

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
    fn clear_device_selections(
        &self,
        device_id: Id,
    ) -> impl Future<Output = ManagementResult<()>>;
}

/// Management Service extension for WebSocket sync selection.
pub trait ManagementSyncSelection {
    /// Select a resource for WebSocket sync on a device.
    async fn select_resource_for_sync(
        &self,
        device_id: Id,
        resource_id: Id,
        application_id: Id,
        owner_subject: &str,
    ) -> ManagementResult<()>;

    /// Deselect a resource for WebSocket sync on a device.
    async fn deselect_resource_for_sync(
        &self,
        device_id: Id,
        resource_id: Id,
    ) -> ManagementResult<()>;

    /// List selected resources for WebSocket sync on a device.
    async fn list_sync_selections_for_device(
        &self,
        device_id: Id,
    ) -> ManagementResult<Vec<SyncSelection>>;

    /// List devices that have a resource selected for WebSocket sync.
    async fn list_sync_targets_for_resource(
        &self,
        resource_id: Id,
    ) -> ManagementResult<Vec<SyncSelection>>;

    /// Get the sync target device for a resource from a source device.
    /// Returns the first device (other than source) that has the resource selected.
    async fn get_sync_target(
        &self,
        resource_id: Id,
        source_device_id: Id,
    ) -> ManagementResult<Option<SyncSelection>>;
}


/// Request to select/deselect a resource for WebSocket sync.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncSelectionRequest {
    pub resource_id: Id,
    pub select: bool,
}

/// Response for sync selection operations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncSelectionResponse {
    pub selections: Vec<SyncSelection>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use idp_model::model::Id;

    #[test]
    fn sync_selection_serialization() {
        let selection = SyncSelection {
            device_id: Id::now_v7(),
            resource_id: Id::now_v7(),
            application_id: Id::now_v7(),
            owner_subject: "user-123".to_string(),
            selected_at: 1234567890,
        };
        let json = serde_json::to_string(&selection).unwrap();
        let deserialized: SyncSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(selection, deserialized);
    }
}
