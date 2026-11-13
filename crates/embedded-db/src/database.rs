use std::path::Path;

use db::{Client, QueryErrorKind};
use of_model::{
    db::{Database, DeviceDatabase, IdpDatabase, ManagementDatabase, StorageDatabase},
    error::DatabaseError,
    replica,
};

#[derive(Clone)]
pub struct EmbeddedDatabase {
    database: db::Database,
}

impl EmbeddedDatabase {
    /// Opens `<data_dir>/unified.redb` and initializes all owner tables.
    pub async fn open(data_dir: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        std::fs::create_dir_all(data_dir.as_ref()).map_err(storage_error)?;
        let database = Self {
            database: crate::local::open_private_database(&data_dir.as_ref().join("unified.redb"))
                .map_err(storage_error)?,
        };
        replica::up(&database).await?;
        Ok(database)
    }
}

#[derive(Clone)]
pub struct MemoryDatabase {
    database: db::Database,
}

impl MemoryDatabase {
    pub async fn new() -> Result<Self, DatabaseError> {
        let database = Self {
            database: db::Database::in_memory(),
        };
        replica::up(&database).await?;
        Ok(database)
    }
}

#[cfg(feature = "network")]
#[derive(Clone)]
pub struct NetworkDatabase {
    client: Client,
}

#[cfg(feature = "network")]
impl NetworkDatabase {
    /// Connects to the existing ofdb SQL service and initializes the shared schema.
    pub async fn connect(config: &db::RemoteDatabaseConfig) -> Result<Self, DatabaseError> {
        let database = Self {
            client: config.connect().await.map_err(|error| {
                let kind = match error.kind() {
                    std::io::ErrorKind::InvalidInput => QueryErrorKind::Validation,
                    std::io::ErrorKind::TimedOut => QueryErrorKind::Timeout,
                    _ => QueryErrorKind::Transport,
                };
                DatabaseError::new(kind, error.to_string())
            })?,
        };
        replica::up(&database).await?;
        Ok(database)
    }
}

fn storage_error(error: std::io::Error) -> DatabaseError {
    DatabaseError::new(QueryErrorKind::Storage, error.to_string())
}

macro_rules! database {
    ($type:ty, $client:expr) => {
        impl IdpDatabase for $type {
            fn client(&self) -> Client {
                ($client)(self)
            }
        }
        impl DeviceDatabase for $type {
            fn client(&self) -> Client {
                ($client)(self)
            }
        }
        impl StorageDatabase for $type {
            fn client(&self) -> Client {
                ($client)(self)
            }
        }
        impl ManagementDatabase for $type {
            fn client(&self) -> Client {
                ($client)(self)
            }
        }
        impl Database for $type {
            fn client(&self) -> Client {
                ($client)(self)
            }
            fn idp(&self) -> &dyn IdpDatabase {
                self
            }
            fn device(&self) -> &dyn DeviceDatabase {
                self
            }
            fn storage(&self) -> &dyn StorageDatabase {
                self
            }
            fn management(&self) -> &dyn ManagementDatabase {
                self
            }
        }
    };
}

database!(EmbeddedDatabase, |database: &EmbeddedDatabase| database
    .database
    .client());
database!(MemoryDatabase, |database: &MemoryDatabase| database
    .database
    .client());
#[cfg(feature = "network")]
database!(NetworkDatabase, |database: &NetworkDatabase| database
    .client
    .clone());
