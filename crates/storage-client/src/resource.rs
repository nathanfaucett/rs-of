use alloc::string::String;
use core::fmt::{Debug, Display};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opaque resource identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceId(pub Uuid);

impl ResourceId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for ResourceId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for ResourceId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<ResourceId> for Uuid {
    fn from(id: ResourceId) -> Self {
        id.0
    }
}

impl Display for ResourceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl core::str::FromStr for ResourceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// Database identifier.
pub type DatabaseId = ResourceId;

/// KV store identifier.
pub type KvStoreId = ResourceId;

/// Filesystem identifier.
pub type FilesystemId = ResourceId;

/// Device identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(pub Uuid);

impl DeviceId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for DeviceId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<DeviceId> for Uuid {
    fn from(id: DeviceId) -> Self {
        id.0
    }
}

impl Display for DeviceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl core::str::FromStr for DeviceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// Application identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApplicationId(pub Uuid);

impl ApplicationId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for ApplicationId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for ApplicationId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<ApplicationId> for Uuid {
    fn from(id: ApplicationId) -> Self {
        id.0
    }
}

impl Display for ApplicationId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl core::str::FromStr for ApplicationId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// User subject identifier.
pub type Subject = String;

/// Sync session handle.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSession {
    pub session_id: String,
    pub resource_id: ResourceId,
    pub target_device: DeviceId,
    pub status: SyncStatus,
}

/// Sync status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Connecting,
    Syncing,
    Completed,
    Failed,
    Cancelled,
}

/// Database info.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub id: DatabaseId,
    pub name: Option<String>,
}

/// KV store info.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KvStoreInfo {
    pub id: KvStoreId,
    pub name: Option<String>,
}

/// Filesystem info.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilesystemInfo {
    pub id: FilesystemId,
    pub name: Option<String>,
}

/// Create database request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateDatabaseRequest {
    pub name: Option<String>,
}

/// Open database request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenDatabaseRequest {
    pub id: DatabaseId,
}

/// Create KV store request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateKvStoreRequest {
    pub name: Option<String>,
}

/// Open KV store request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenKvStoreRequest {
    pub id: KvStoreId,
}

/// Create filesystem request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateFilesystemRequest {
    pub name: Option<String>,
}

/// Open filesystem request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenFilesystemRequest {
    pub id: FilesystemId,
}

/// Sync to device request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncToDeviceRequest {
    pub resource_id: ResourceId,
    pub target_device: DeviceId,
}
