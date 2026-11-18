use std::io;
use std::path::Path;

use btree_redb::SharedDatabase;
use ofdb_sql::Client;
use ofdb_sql::Database as SqlDatabase;
use ofdb_sql::QueryError;
use ofdb_sql::QueryErrorKind;
use thiserror::Error;

use crate::local::open_private_storage;
use crate::ofdb::up;

/// Error type for adapter operations.
#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("storage error: {0}")]
    Storage(#[from] io::Error),
    #[error("SQL error: {0}")]
    Sql(#[from] QueryError),
}

impl AdapterError {
    #[must_use]
    pub const fn storage_error(error: io::Error) -> Self {
        Self::Storage(error)
    }

    #[must_use]
    pub fn sql_error(kind: QueryErrorKind, message: String) -> Self {
        Self::Sql(QueryError::new(kind, message))
    }
}

/// File-backed embedded database (redb).
#[derive(Clone)]
pub struct EmbeddedDatabase {
    pub database: SqlDatabase,
    pub storage: SharedDatabase,
}

impl EmbeddedDatabase {
    /// Opens `<data_dir>/unified.redb` and initializes all owner tables.
    pub async fn open(data_dir: impl AsRef<Path>) -> Result<Self, AdapterError> {
        std::fs::create_dir_all(data_dir.as_ref())?;
        Self::open_file(data_dir.as_ref().join("unified.redb")).await
    }

    /// Opens a specific database file and initializes all owner tables.
    pub async fn open_file(path: impl AsRef<Path>) -> Result<Self, AdapterError> {
        let storage = open_private_storage(path.as_ref())?;
        let database = Self {
            database: SqlDatabase::open_shared(&storage, "control")?,
            storage,
        };
        up(&database.client()).await?;
        Ok(database)
    }

    /// Returns the SQL client for query access.
    #[must_use]
    pub fn client(&self) -> Client {
        self.database.client()
    }

    /// Returns the underlying redb storage (for filesystem runtime).
    #[must_use]
    pub fn storage(&self) -> SharedDatabase {
        self.storage.clone()
    }
}

/// In-memory database (redb in-memory).
#[derive(Clone)]
pub struct MemoryDatabase {
    database: SqlDatabase,
    storage: SharedDatabase,
}

impl MemoryDatabase {
    /// Creates a new in-memory database and initializes all owner tables.
    pub async fn new() -> Result<Self, AdapterError> {
        let storage = SharedDatabase::in_memory()?;
        let database = Self {
            database: SqlDatabase::open_shared(&storage, "control")?,
            storage,
        };
        up(&database.client()).await?;
        Ok(database)
    }

    /// Returns the SQL client for query access.
    #[must_use]
    pub fn client(&self) -> Client {
        self.database.client()
    }

    /// Returns the underlying redb storage.
    #[must_use]
    pub fn storage(&self) -> SharedDatabase {
        self.storage.clone()
    }

    /// Returns the underlying SQL database.
    #[must_use]
    pub fn database(&self) -> SqlDatabase {
        self.database.clone()
    }
}

/// Remote ofdb SQL service database (network transport).
#[cfg(feature = "remote")]
#[derive(Clone)]
pub struct RemoteDatabase {
    client: Client,
}

#[cfg(feature = "remote")]
impl RemoteDatabase {
    /// Connects to the existing ofdb SQL service and initializes the shared schema.
    pub async fn connect(
        config: &crate::remote::RemoteDatabaseConfig,
    ) -> Result<Self, AdapterError> {
        let client = config.connect().await.map_err(|error| {
            let kind = match error.kind() {
                std::io::ErrorKind::InvalidInput => QueryErrorKind::Validation,
                std::io::ErrorKind::TimedOut => QueryErrorKind::Timeout,
                _ => QueryErrorKind::Transport,
            };
            AdapterError::sql_error(kind, error.to_string())
        })?;
        let database = Self { client };
        up(&database.client).await?;
        Ok(database)
    }

    /// Returns the SQL client for query access.
    #[must_use]
    pub fn client(&self) -> Client {
        self.client.clone()
    }
}

/// PostgreSQL network database.
#[cfg(feature = "postgres")]
#[derive(Clone)]
pub struct NetworkDatabase {
    client: Client,
}

#[cfg(feature = "postgres")]
impl NetworkDatabase {
    /// Connects to PostgreSQL and initializes all owner tables.
    /// Requires explicit `sslmode=disable`; use a Unix socket or trusted TLS tunnel.
    pub async fn connect(config: &str) -> Result<Self, AdapterError> {
        let client = Client::connect_postgres(config).await?;
        let database = Self { client };
        up(&database.client).await?;
        Ok(database)
    }

    /// Returns the SQL client for query access.
    pub fn client(&self) -> Client {
        self.client.clone()
    }
}
