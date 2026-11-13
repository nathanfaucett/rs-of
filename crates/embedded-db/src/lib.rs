#![forbid(unsafe_code)]

mod database;

#[cfg(feature = "network")]
pub use database::NetworkDatabase;
pub use database::{EmbeddedDatabase, MemoryDatabase};
#[cfg(feature = "network")]
pub use db::RemoteDatabaseConfig;
