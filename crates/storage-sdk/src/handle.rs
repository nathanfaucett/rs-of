use alloc::string::String;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a database resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DatabaseId(pub Uuid);

impl DatabaseId {
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

impl Default for DatabaseId {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for DatabaseId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for DatabaseId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<DatabaseId> for Uuid {
    fn from(id: DatabaseId) -> Self {
        id.0
    }
}

/// Unique identifier for a KV store resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KvStoreId(pub Uuid);

impl KvStoreId {
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

impl Default for KvStoreId {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for KvStoreId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for KvStoreId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<KvStoreId> for Uuid {
    fn from(id: KvStoreId) -> Self {
        id.0
    }
}

/// Unique identifier for a filesystem resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FilesystemId(pub Uuid);

impl FilesystemId {
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

impl Default for FilesystemId {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for FilesystemId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for FilesystemId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<FilesystemId> for Uuid {
    fn from(id: FilesystemId) -> Self {
        id.0
    }
}

/// Generic resource ID that can represent any storage resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceId(pub Uuid);

impl ResourceId {
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

impl Default for ResourceId {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for ResourceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for ResourceId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<ResourceId> for Uuid {
    fn from(id: ResourceId) -> Self {
        id.0
    }
}

impl From<DatabaseId> for ResourceId {
    fn from(id: DatabaseId) -> Self {
        Self(id.0)
    }
}

impl From<KvStoreId> for ResourceId {
    fn from(id: KvStoreId) -> Self {
        Self(id.0)
    }
}

impl From<FilesystemId> for ResourceId {
    fn from(id: FilesystemId) -> Self {
        Self(id.0)
    }
}

/// Information about a database resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub id: DatabaseId,
    pub name: String,
    pub application_id: String,
}

/// Information about a KV store resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvStoreInfo {
    pub id: KvStoreId,
    pub name: String,
    pub application_id: String,
}

/// Information about a filesystem resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemInfo {
    pub id: FilesystemId,
    pub name: String,
    pub application_id: String,
}
