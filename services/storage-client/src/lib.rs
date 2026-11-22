//! Storage Client — unified API for creating and syncing SQL, KV, and filesystem resources.
//!
//! This crate provides high-level handles that wrap local engines (ofdb-sql, ofdb-kv, offs)
//! with built-in sync transports (WebSocket for browser/server, Iroh for native).

#![cfg_attr(not(feature = "native"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

#[cfg(feature = "native")]
extern crate std;

pub mod client;
pub mod error;
pub mod handle;
pub mod protocol;
pub mod transport;

#[cfg(feature = "native")]
pub mod native;

#[cfg(feature = "browser")]
pub mod browser;

use core::{
    fmt::{self, Debug, Display},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opaque resource identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceId(pub Uuid);

impl ResourceId {
    #[must_use]
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl FromStr for ResourceId {
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
    #[must_use]
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl FromStr for DeviceId {
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
    #[must_use]
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl FromStr for ApplicationId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// User subject identifier.
pub type Subject = String;

/// Sync session result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncResult {
    pub received_units: usize,
    pub sent_units: usize,
    pub received_changes: usize,
    pub sent_changes: usize,
    pub received_snapshots: usize,
    pub sent_snapshots: usize,
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
