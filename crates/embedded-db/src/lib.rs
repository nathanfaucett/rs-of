#![forbid(unsafe_code)]

pub mod database;
pub mod local;

pub use local::local_client;

#[cfg(feature = "network")]
pub use database::NetworkDatabase;
pub use database::{EmbeddedDatabase, MemoryDatabase};
#[cfg(feature = "network")]
pub use db::RemoteDatabaseConfig;
