/// Resource handles with built-in sync transport.

use alloc::{boxed::Box, string::ToString};
use core::fmt::{Debug, Formatter, Result as FmtResult};
use core::future::Future;
use uuid::Uuid;

use crate::error::{Result, StorageClientError};
use crate::transport::{TransportConfig, WsSyncTransport};

#[cfg(feature = "native")]
use crate::transport::NativeWsTransport;

#[cfg(feature = "browser")]
use crate::transport::BrowserWsTransport;

use ofdb_sql::{Database, QueryParams, QueryResult, SessionConfig, SyncResult as OfdbSyncResult, SyncRole};
use storage_sync_client::client::{TokenProvider, WsTransportBridge};
use storage_sync_client::protocol::StoreIdentity;

/// SQL Database handle with built-in sync transport.
pub struct SqlDatabaseHandle {
    database: Database,
    store_identity: StoreIdentity,
    transport_config: TransportConfig,
    _token_provider: TokenProvider,
    _local_replica_id: Uuid,
}

impl SqlDatabaseHandle {
    /// Create a new SQL database handle.
    pub fn new(
        database: Database,
        store_identity: StoreIdentity,
        transport_config: TransportConfig,
        token_provider: TokenProvider,
        local_replica_id: Uuid,
    ) -> Self {
        Self {
            database,
            store_identity,
            transport_config,
            _token_provider: token_provider,
            _local_replica_id: local_replica_id,
        }
    }

    /// Get the underlying database.
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Get a mutable reference to the database.
    pub fn database_mut(&mut self) -> &mut Database {
        &mut self.database
    }

    /// Execute SQL directly on the database.
    pub async fn execute(&self, sql: &str) -> Result<QueryResult> {
        let client = self.database.client();
        let results = client
            .execute_sql(sql, None)
            .await
            .map_err(|e| StorageClientError::Database(e.to_string()))?;
        results.into_iter().next().ok_or(StorageClientError::Database("no result".into()))
    }

    /// Execute SQL with parameters.
    pub async fn execute_with_params(
        &self,
        sql: &str,
        params: QueryParams,
    ) -> Result<QueryResult> {
        let client = self.database.client();
        let results = client
            .execute_sql(sql, Some(&params))
            .await
            .map_err(|e| StorageClientError::Database(e.to_string()))?;
        results.into_iter().next().ok_or(StorageClientError::Database("no result".into()))
    }

    /// Start a transaction.
    pub async fn transaction(&self) -> Result<ofdb_sql::Transaction> {
        self.database
            .client()
            .transaction()
            .await
            .map_err(|e| StorageClientError::Database(e.to_string()))
    }

    /// Synchronize with a remote peer using the built-in transport.
    pub async fn synchronize(
        &mut self,
        config: Option<SessionConfig>,
        role: SyncRole,
    ) -> Result<OfdbSyncResult> {
        let mut transport = self.create_transport().await?;
        let config = config.unwrap_or_default();

        let result = self
            .database
            .synchronize(&mut transport, &config, role)
            .await
            .map_err(|e| StorageClientError::Sync(e.to_string()))?;

        Ok(result)
    }

    /// Synchronize with authorization check before each batch.
    pub async fn synchronize_checked<F, Fut>(
        &mut self,
        config: Option<SessionConfig>,
        role: SyncRole,
        check: F,
    ) -> Result<OfdbSyncResult>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send,
    {
        let mut transport = self.create_transport().await?;
        let config = config.unwrap_or_default();

        let check_fn = move || {
            let fut = check();
            async move {
                fut.await.map_err(|e| e.to_string())
            }
        };

        let result = self
            .database
            .synchronize_checked(&mut transport, &config, role, check_fn)
            .await
            .map_err(|e| StorageClientError::Sync(e.to_string()))?;

        Ok(result)
    }

    /// Get the store identity for this database.
    pub fn store_identity(&self) -> &StoreIdentity {
        &self.store_identity
    }

    /// Get the resource ID.
    pub fn resource_id(&self) -> Uuid {
        self.store_identity.store_id
    }

    async fn create_transport(&self) -> Result<WsTransportBridge<Box<dyn WsSyncTransport + Send>>> {
        #[cfg(feature = "native")]
        {
            let transport = NativeWsTransport::connect(&self.transport_config.sync_ws_url).await?;
            let bridge = WsTransportBridge::new(Box::new(transport) as Box<dyn WsSyncTransport + Send>);
            Ok(bridge)
        }

        #[cfg(feature = "browser")]
        {
            let transport = BrowserWsTransport::connect(&self.transport_config.sync_ws_url, &self.transport_config.auth_token).await?;
            let bridge = WsTransportBridge::new(Box::new(transport) as Box<dyn WsSyncTransport + Send>);
            Ok(bridge)
        }

        #[cfg(not(any(feature = "native", feature = "browser")))]
        {
            Err(StorageClientError::InvalidArgument(
                "No transport feature enabled".into(),
            ))
        }
    }
}

impl Debug for SqlDatabaseHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("SqlDatabaseHandle")
            .field("resource_id", &self.resource_id())
            .finish()
    }
}

/// KV Store handle with built-in sync transport.
pub struct KvStoreHandle {
    // TODO: Add ofdb-kv database when KV sync is implemented
    store_identity: StoreIdentity,
    _transport_config: TransportConfig,
    _token_provider: TokenProvider,
    _local_replica_id: Uuid,
}

impl KvStoreHandle {
    pub fn new(
        store_identity: StoreIdentity,
        transport_config: TransportConfig,
        token_provider: TokenProvider,
        local_replica_id: Uuid,
    ) -> Self {
        Self {
            store_identity,
            _transport_config: transport_config,
            _token_provider: token_provider,
            _local_replica_id: local_replica_id,
        }
    }

    /// Get the store identity for this KV store.
    pub fn store_identity(&self) -> &StoreIdentity {
        &self.store_identity
    }

    /// Get the resource ID.
    pub fn resource_id(&self) -> Uuid {
        self.store_identity.store_id
    }
}

impl Debug for KvStoreHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("KvStoreHandle")
            .field("resource_id", &self.resource_id())
            .finish()
    }
}

/// Filesystem handle with built-in sync transport.
pub struct FilesystemHandle {
    // TODO: Add file-system instance when filesystem sync is implemented
    store_identity: StoreIdentity,
    _transport_config: TransportConfig,
    _token_provider: TokenProvider,
    _local_replica_id: Uuid,
}

impl FilesystemHandle {
    pub fn new(
        store_identity: StoreIdentity,
        transport_config: TransportConfig,
        token_provider: TokenProvider,
        local_replica_id: Uuid,
    ) -> Self {
        Self {
            store_identity,
            _transport_config: transport_config,
            _token_provider: token_provider,
            _local_replica_id: local_replica_id,
        }
    }

    /// Get the store identity for this filesystem.
    pub fn store_identity(&self) -> &StoreIdentity {
        &self.store_identity
    }

    /// Get the resource ID.
    pub fn resource_id(&self) -> Uuid {
        self.store_identity.store_id
    }
}

impl Debug for FilesystemHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("FilesystemHandle")
            .field("resource_id", &self.resource_id())
            .finish()
    }
}
