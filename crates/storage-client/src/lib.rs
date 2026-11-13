#![cfg_attr(not(any(feature = "native", feature = "browser")), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;
extern crate std;

pub mod error;
pub mod handle;
pub mod protocol;
mod resource;
pub mod transport;
pub use ofdb_sql::history::{SessionConfig, SyncResult, SyncRole};
pub use resource::{
    ApplicationId, CreateDatabaseRequest, CreateFilesystemRequest, CreateKvStoreRequest,
    DatabaseId, DatabaseInfo, DeviceId, FilesystemId, FilesystemInfo, KvStoreId, KvStoreInfo,
    OpenDatabaseRequest, OpenFilesystemRequest, OpenKvStoreRequest, ResourceId, Subject,
    SyncSession, SyncStatus, SyncToDeviceRequest,
};

#[cfg(feature = "browser")]
pub mod browser;
#[cfg(any(feature = "native", feature = "browser"))]
mod client;
#[cfg(feature = "native")]
pub mod native;
