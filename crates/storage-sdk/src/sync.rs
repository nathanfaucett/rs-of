
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ResourceId;

/// Represents an active sync session between two devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSession {
    pub session_id: Uuid,
    pub resource_id: ResourceId,
    pub source_device: DeviceId,
    pub target_device: DeviceId,
    pub status: SyncStatus,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Status of a sync session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncStatus {
    Connecting,
    Syncing,
    Completed,
    Failed,
    Cancelled,
}

/// Configuration for sync to device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncToDeviceConfig {
    pub resource_id: ResourceId,
    pub target_device: DeviceId,
    // Direction removed - all transport is bidirectional
}

/// Sync direction - all transport is bidirectional, but kept for API compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncDirection {
    Bidirectional,
}

/// Device identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(pub Uuid);

impl DeviceId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for DeviceId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<DeviceId> for Uuid {
    fn from(id: DeviceId) -> Self {
        id.0
    }
}

/// Result of a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub session_id: Uuid,
    pub status: SyncStatus,
    pub changes_sent: u64,
    pub changes_received: u64,
    pub conflicts_resolved: u64,
    pub duration_ms: u64,
}

impl Default for SyncResult {
    fn default() -> Self {
        Self {
            session_id: Uuid::nil(),
            status: SyncStatus::Completed,
            changes_sent: 0,
            changes_received: 0,
            conflicts_resolved: 0,
            duration_ms: 0,
        }
    }
}
