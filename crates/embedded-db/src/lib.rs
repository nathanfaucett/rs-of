#![forbid(unsafe_code)]

mod database;
mod local;

pub use local::{local_client, local_database};

#[cfg(feature = "postgres")]
pub use database::NetworkDatabase;
#[cfg(feature = "network")]
pub use database::RemoteDatabase;
pub use database::{EmbeddedDatabase, MemoryDatabase};
#[cfg(feature = "network")]
pub use db::RemoteDatabaseConfig;
