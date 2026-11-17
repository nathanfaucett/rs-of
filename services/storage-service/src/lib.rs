// Shorter namespace aliases for use within this crate
pub use file_system as fs;
pub use of_model as model;
pub use ofdb_kv as db_kv;
pub use ofdb_sql as db_sql;

pub mod database_catalog;
pub mod database_runtime;
mod local_copy;
mod persistence;
pub mod scoped_file_system;
pub mod service;

pub use database_catalog::{DatabaseCatalog, DatabaseId, DatabaseResource};
pub use database_runtime::DatabaseRuntime;
pub use file_system::{FileSystemId, FileSystemResource};
pub use of_model::storage::contract::{StorageErrorCode, StorageRequest, StorageResponse};
pub use of_model::storage::namespace::StorageNamespace;
pub use persistence::{error as persistence_error, run as persistence_run};
pub use scoped_file_system::{ScopedFileSystem, ScopedFileSystemRuntime};
pub use service::{ScopedStorageService, StorageService, StorageServiceError};
