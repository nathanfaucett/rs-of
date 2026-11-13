#![allow(dead_code)]

#[path = "../../../../crates/storage-service/src/database_catalog.rs"]
mod database_catalog;
#[path = "../../../../crates/storage-service/src/database_runtime.rs"]
mod database_runtime;
mod identity;
#[path = "../../../../crates/storage-service/src/local_copy.rs"]
mod local_copy;
#[path = "../../../../crates/idp-server/src/local_seed.rs"]
mod local_seed;
#[cfg(test)]
mod tests;

pub use database_catalog::{DatabaseCatalog, DatabaseId, DatabaseResource};
pub use database_runtime::DatabaseRuntime;
pub use identity::RouterState;
