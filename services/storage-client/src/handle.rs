use alloc::string::{String, ToString};
use core::future::Future;

use ofdb_sql::history::{SessionConfig, SyncResult, SyncRole, SyncTransport};
use ofdb_sql::{Database, QueryParams, QueryResult};
use uuid::Uuid;

use crate::{
    error::{Result, StorageClientError},
    protocol::StoreIdentity,
    transport::{TokenProvider, TransportConfig},
};

pub struct SqlDatabaseHandle {
    database: Database,
    store_identity: StoreIdentity,
    transport_config: TransportConfig,
    token_provider: TokenProvider,
}
impl SqlDatabaseHandle {
    pub fn new(
        database: Database,
        store_identity: StoreIdentity,
        transport_config: TransportConfig,
        token_provider: TokenProvider,
    ) -> Self {
        Self {
            database,
            store_identity,
            transport_config,
            token_provider,
        }
    }
    pub fn database(&self) -> &Database {
        &self.database
    }
    pub fn database_mut(&mut self) -> &mut Database {
        &mut self.database
    }
    pub async fn execute(&self, sql: &str) -> Result<QueryResult> {
        self.database
            .client()
            .execute_sql(sql, None)
            .await
            .map_err(|e| StorageClientError::Database(e.to_string()))?
            .into_iter()
            .next()
            .ok_or_else(|| StorageClientError::Database("no result".into()))
    }
    pub async fn execute_with_params(&self, sql: &str, params: QueryParams) -> Result<QueryResult> {
        self.database
            .client()
            .execute_sql(sql, Some(&params))
            .await
            .map_err(|e| StorageClientError::Database(e.to_string()))?
            .into_iter()
            .next()
            .ok_or_else(|| StorageClientError::Database("no result".into()))
    }
    pub async fn transaction(&self) -> Result<ofdb_sql::Transaction> {
        self.database
            .client()
            .transaction()
            .await
            .map_err(|e| StorageClientError::Database(e.to_string()))
    }
    /// Exchanges typed SQL history on an owner-admitted resource stream.
    /// The check must reject revoked authorization or cancellation before each apply.
    pub async fn synchronize_with<T, Q, U>(
        &self,
        transport: &mut T,
        config: &SessionConfig,
        role: SyncRole,
        staging_directory: impl AsRef<std::path::Path>,
        check: Q,
    ) -> Result<SyncResult>
    where
        T: SyncTransport,
        T::Error: core::fmt::Display,
        Q: FnMut() -> U + ofdb_platform::MaybeSend,
        U: Future<Output = core::result::Result<(), String>> + ofdb_platform::MaybeSend,
    {
        self.database
            .exchange_resource_history(transport, config, role, staging_directory, check)
            .await
            .map_err(|e| StorageClientError::Sync(e.to_string()))
    }
    #[cfg(any(
        all(feature = "native", not(target_arch = "wasm32")),
        all(feature = "browser", target_arch = "wasm32")
    ))]
    pub async fn synchronize(&self, config: Option<SessionConfig>) -> Result<SyncResult> {
        let token = (self.token_provider)().await?;
        let mut transport = crate::transport::OwnerTransport::connect(
            &self.transport_config.sync_ws_url,
            &self.store_identity,
            token,
        )
        .await?;
        let directory = std::env::temp_dir()
            .join("of-storage-client")
            .join(self.resource_id().to_string())
            .join(self.store_identity.replica_id.to_string());
        self.synchronize_with(
            &mut transport,
            &config.unwrap_or_default(),
            SyncRole::Initiator,
            directory,
            || async {
                (self.token_provider)()
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            },
        )
        .await
    }
    pub fn store_identity(&self) -> &StoreIdentity {
        &self.store_identity
    }
    pub fn resource_id(&self) -> Uuid {
        self.store_identity.store_id
    }
}
impl core::fmt::Debug for SqlDatabaseHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SqlDatabaseHandle")
            .field("resource_id", &self.resource_id())
            .finish()
    }
}

pub struct KvStoreHandle {
    database: ofdb_kv::Database,
    store_identity: StoreIdentity,
}
impl KvStoreHandle {
    pub fn new(database: ofdb_kv::Database, store_identity: StoreIdentity) -> Self {
        Self {
            database,
            store_identity,
        }
    }
    pub fn database(&self) -> &ofdb_kv::Database {
        &self.database
    }
    /// Exchanges KV v3 history on an owner-admitted resource stream.
    pub async fn synchronize_with<T, Q, U>(
        &self,
        transport: &mut T,
        role: ofdb_kv::history::SyncRole,
        config: ofdb_kv::history::Config,
        check: Q,
    ) -> Result<()>
    where
        T: ofdb_kv::history::SyncTransport,
        T::Error: core::fmt::Display,
        Q: FnMut() -> U,
        U: Future<Output = core::result::Result<(), String>>,
    {
        self.database
            .exchange_history(transport, role, config, check)
            .await
            .map_err(|e| StorageClientError::Sync(e.to_string()))
    }
    pub fn store_identity(&self) -> &StoreIdentity {
        &self.store_identity
    }
    pub fn resource_id(&self) -> Uuid {
        self.store_identity.store_id
    }
}
impl core::fmt::Debug for KvStoreHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KvStoreHandle")
            .field("resource_id", &self.resource_id())
            .finish()
    }
}

pub struct FilesystemHandle {
    store_identity: StoreIdentity,
}
impl FilesystemHandle {
    pub fn new(store_identity: StoreIdentity) -> Self {
        Self { store_identity }
    }
    pub fn store_identity(&self) -> &StoreIdentity {
        &self.store_identity
    }
    pub fn resource_id(&self) -> Uuid {
        self.store_identity.store_id
    }
}
impl core::fmt::Debug for FilesystemHandle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FilesystemHandle")
            .field("resource_id", &self.resource_id())
            .finish()
    }
}
