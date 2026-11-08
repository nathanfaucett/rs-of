#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! High-level storage SDK for the of storage service.
//!
//! This crate provides typed handles for SQL databases, KV stores, and filesystems
//! with built-in sync transport capabilities. It wraps the generated storage-client
//! (HTTP REST API), storage-sync-client (WebSocket transport), and local engines
//! (ofdb-sql, ofdb-kv, offs).

#[cfg(feature = "native")]
extern crate std;

extern crate alloc;

pub mod error;
pub mod handle;
pub mod sync;

#[cfg(feature = "native")]
pub mod native;

#[cfg(feature = "browser")]
pub mod browser;

pub use error::{StorageSdkError, StorageSdkResult};
pub use handle::{DatabaseId, DatabaseInfo, FilesystemId, FilesystemInfo, KvStoreId, KvStoreInfo, ResourceId};
pub use sync::{DeviceId, SyncDirection, SyncResult, SyncSession, SyncStatus, SyncToDeviceConfig};

#[cfg(feature = "native")]
pub use native::StorageSdk;

#[cfg(feature = "browser")]
pub use browser::StorageSdk;
