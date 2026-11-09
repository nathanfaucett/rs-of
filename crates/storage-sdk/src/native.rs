#![allow(unused_imports)]

#[cfg(feature = "native")]
use {
    alloc::{string::String, vec::Vec},
    ofdb_sql::{
        Database as SqlDatabase, SessionConfig, SyncResult as OfdbSyncResult, SyncRole, SyncTransport,
        QueryParams, QueryResult,
        Client as SqlClient, Transaction as SqlTransaction,
    },
    ofdb_value::Value as OfdbValue,
    of_storage_api_client::apis::configuration::Configuration,
    of_storage_api_client::apis::resources_api::{
        create_database, create_file_system, delete_database,
        get_database, get_file_system, list_databases, list_file_systems,
        GetDatabaseParams, GetFileSystemParams,
    },
    of_storage_api_client::apis::sync_api::sync_connect,
    of_storage_api_client::models::{SyncConnectRequest, SyncSessionResponse, Status, CreateResourceRequest},
    storage_sync_client::{
        native::WebSocketSyncTransport, protocol::StoreIdentity, TokenProvider,
        client::{WsTransportBridge, WsTransportBridgeShared},
        transport::TransportError,
    },
    std::sync::{Arc, Mutex},
    tokio::sync::Mutex as TokioMutex,
    uuid::Uuid,
};

#[cfg(feature = "native")]
use crate::{
    error::{StorageSdkError, StorageSdkResult},
    handle::{ResourceId,
        DatabaseId, DatabaseInfo, FilesystemId, FilesystemInfo, KvStoreId,
        KvStoreInfo,
    },
    sync::{DeviceId, SyncDirection as ClientSyncDirection, SyncResult, SyncSession, SyncStatus, SyncToDeviceConfig},
};

/// Type alias for the token provider function.
#[cfg(feature = "native")]
type BoxTokenProvider = Box<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, storage_sync_client::SyncError>> + Send>> + Send + Sync>;

#[cfg(feature = "native")]
#[derive(Clone)]
pub struct StorageSdk {
    http_config: Arc<Mutex<Configuration>>,
    token_provider: Arc<BoxTokenProvider>,
    application_id: String,
    _user_sub: String,
    device_id: DeviceId,
}

#[cfg(feature = "native")]
impl StorageSdk {
    /// Create a new storage SDK.
    pub fn new(
        base_url: impl Into<String>,
        token_provider: impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, storage_sync_client::SyncError>> + Send>> + Send + Sync + 'static,
        application_id: impl Into<String>,
        user_sub: impl Into<String>,
        device_id: DeviceId,
    ) -> Self {
        let mut http_config = Configuration::new();
        http_config.base_path = base_url.into();

        Self {
            http_config: Arc::new(Mutex::new(http_config)),
            token_provider: Arc::new(Box::new(token_provider)),
            application_id: application_id.into(),
            _user_sub: user_sub.into(),
            device_id,
        }
    }

    async fn with_config<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Configuration) -> R,
    {
        let mut config = self.http_config.lock().unwrap();
        f(&mut config)
    }

    /// Create a new SQL database.
    pub async fn create_sql_db(&self, name: &str) -> StorageSdkResult<SqlDatabaseHandle> {
        let token = (self.token_provider)().await?;
        self.with_config(|config| config.bearer_access_token = Some(token.into())).await;

        let request = of_storage_api_client::apis::resources_api::CreateDatabaseParams {
            create_resource_request: CreateResourceRequest { name: Some(Some(name.to_string())) },
        };
        let db_info = create_database(&self.http_config.lock().unwrap(), request).await?;

        let database = self.open_local_database(&DatabaseId::from_uuid(Uuid::parse_str(&db_info.id).unwrap())).await?;

        Ok(SqlDatabaseHandle {
            sdk: self.clone(),
            info: DatabaseInfo {
                id: DatabaseId::from_uuid(Uuid::parse_str(&db_info.id).unwrap()),
                name: db_info.name.flatten().unwrap_or_default(),
                application_id: self.application_id.clone(),
            },
            database: Arc::new(TokioMutex::new(Some(database))),
        })
    }

    /// Open an existing SQL database by ID.
    pub async fn open_sql_db(&self, id: DatabaseId) -> StorageSdkResult<SqlDatabaseHandle> {
        let token = (self.token_provider)().await?;
        self.with_config(|config| config.bearer_access_token = Some(token.into())).await;

        let db_info = get_database(
            &self.http_config.lock().unwrap(),
            of_storage_api_client::apis::resources_api::GetDatabaseParams { database_id: id.to_string() },
        ).await?;

        let database = self.open_local_database(&id).await?;

        Ok(SqlDatabaseHandle {
            sdk: self.clone(),
            info: DatabaseInfo {
                id: DatabaseId::from_uuid(Uuid::parse_str(&db_info.id).unwrap()),
                name: db_info.name.flatten().unwrap_or_default(),
                application_id: db_info.application_id.clone(),
            },
            database: Arc::new(TokioMutex::new(Some(database))),
        })
    }

    /// List all SQL databases.
    pub async fn list_sql_dbs(&self) -> StorageSdkResult<Vec<DatabaseInfo>> {
        let token = (self.token_provider)().await?;
        self.with_config(|config| config.bearer_access_token = Some(token.into())).await;

        let databases = list_databases(&self.http_config.lock().unwrap()).await?;

        Ok(databases.into_iter().map(|d| DatabaseInfo {
            id: DatabaseId::from_uuid(Uuid::parse_str(&d.id).unwrap()),
            name: d.name.flatten().unwrap_or_default(),
            application_id: self.application_id.clone(),
        }).collect())
    }

    /// Create a new KV store.
    pub async fn create_kv_store(&self, _name: &str) -> StorageSdkResult<KvStoreHandle> {
        Err(StorageSdkError::other("KV store not yet implemented in generated client"))
    }

    /// Open an existing KV store by ID.
    pub async fn open_kv_store(&self, _id: KvStoreId) -> StorageSdkResult<KvStoreHandle> {
        Err(StorageSdkError::other("KV store not yet implemented in generated client"))
    }

    /// List all KV stores.
    pub async fn list_kv_stores(&self) -> StorageSdkResult<Vec<KvStoreInfo>> {
        Err(StorageSdkError::other("KV store not yet implemented in generated client"))
    }

    /// Create a new filesystem.
    pub async fn create_filesystem(&self, name: &str) -> StorageSdkResult<FilesystemHandle> {
        let token = (self.token_provider)().await?;
        self.with_config(|config| config.bearer_access_token = Some(token.into())).await;

        let request = of_storage_api_client::apis::resources_api::CreateFileSystemParams {
            create_resource_request: CreateResourceRequest { name: Some(Some(name.to_string())) },
        };
        let info = create_file_system(&self.http_config.lock().unwrap(), request).await?;

        let fs = self.open_local_filesystem(&FilesystemId::from_uuid(Uuid::parse_str(&info.id).unwrap())).await?;

        Ok(FilesystemHandle {
            sdk: self.clone(),
            info: FilesystemInfo {
                id: FilesystemId::from_uuid(Uuid::parse_str(&info.id).unwrap()),
                name: info.name.flatten().unwrap_or_default(),
                application_id: self.application_id.clone(),
            },
            filesystem: Arc::new(TokioMutex::new(Some(fs))),
        })
    }

    /// Open an existing filesystem by ID.
    pub async fn open_filesystem(&self, id: FilesystemId) -> StorageSdkResult<FilesystemHandle> {
        let token = (self.token_provider)().await?;
        self.with_config(|config| config.bearer_access_token = Some(token.into())).await;

        let info = get_file_system(
            &self.http_config.lock().unwrap(),
            GetFileSystemParams { filesystem_id: id.to_string() },
        ).await?;

        let fs = self.open_local_filesystem(&id).await?;

        Ok(FilesystemHandle {
            sdk: self.clone(),
            info: FilesystemInfo {
                id: FilesystemId::from_uuid(Uuid::parse_str(&info.id).unwrap()),
                name: info.name.flatten().unwrap_or_default(),
                application_id: info.application_id,
            },
            filesystem: Arc::new(TokioMutex::new(Some(fs))),
        })
    }

    /// List all filesystems.
    pub async fn list_filesystems(&self) -> StorageSdkResult<Vec<FilesystemInfo>> {
        let token = (self.token_provider)().await?;
        self.with_config(|config| config.bearer_access_token = Some(token.into())).await;

        let filesystems = list_file_systems(&self.http_config.lock().unwrap()).await?;

        Ok(filesystems.into_iter().map(|f| FilesystemInfo {
            id: FilesystemId::from_uuid(Uuid::parse_str(&f.id).unwrap()),
            name: f.name.flatten().unwrap_or_default(),
            application_id: self.application_id.clone(),
        }).collect())
    }

    /// Start a sync session to another device.
    pub async fn sync_to_device(
        &self,
        config: SyncToDeviceConfig,
    ) -> StorageSdkResult<SyncSession> {
        let token = (self.token_provider)().await?;
        self.with_config(|config| config.bearer_access_token = Some(token.into())).await;

        let request = SyncConnectRequest {
            resource_id: config.resource_id.into(),
            target_device: config.target_device.into(),
        };

        let session = sync_connect(&self.http_config.lock().unwrap(), of_storage_api_client::apis::sync_api::SyncConnectParams {
            sync_connect_request: request,
        }).await?;

        let status = match session.status {
            Status::Connecting => SyncStatus::Connecting,
            Status::Syncing => SyncStatus::Syncing,
            Status::Completed => SyncStatus::Completed,
            Status::Failed => SyncStatus::Failed,
            Status::Cancelled => SyncStatus::Cancelled,
        };

        Ok(SyncSession {
            session_id: session.session_id,
            resource_id: ResourceId::from_uuid(session.resource_id),
            source_device: DeviceId::from_uuid(session.source_device),
            target_device: DeviceId::from_uuid(session.target_device),
            status,
            bytes_sent: session.bytes_sent as u64,
            bytes_received: session.bytes_received as u64,
        })
    }

    async fn open_local_database(&self, id: &DatabaseId) -> StorageSdkResult<SqlDatabase> {
        let path = format!("./data/databases/{}.redb", id);
        std::fs::create_dir_all("./data/databases").ok();

        let database = SqlDatabase::open(&path)
            .map_err(|e| StorageSdkError::database(e.to_string()))?;

        Ok(database)
    }

    async fn _open_local_kv_store(&self, id: &KvStoreId) -> StorageSdkResult<ofdb_kv::Client> {
        let path = format!("./data/kv/{}.redb", id);
        std::fs::create_dir_all("./data/kv").ok();

        let database = ofdb_kv::Database::open(&path)
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        let client = database.client();

        Ok(client)
    }

    async fn open_local_filesystem(&self, id: &FilesystemId) -> StorageSdkResult<file_system::FileSystem> {
        let path = format!("./data/filesystems/{}", id);
        std::fs::create_dir_all(&path).ok();

        let node_id = Uuid::nil();
        let fs = file_system::FileSystem::open(&path, node_id)
            .await
            .map_err(|e| StorageSdkError::filesystem(e.to_string()))?;

        Ok(fs)
    }

    fn create_store_identity(&self, resource_id: ResourceId) -> StoreIdentity {
        let store_id = Uuid::from_u128(resource_id.0.as_u128());
        let replica_id = Uuid::from_u128(self.device_id.0.as_u128());
        let mut schema_fingerprint = [0u8; 32];
        schema_fingerprint[..16].copy_from_slice(&resource_id.0.as_u128().to_le_bytes());
        schema_fingerprint[16..].copy_from_slice(&self.device_id.0.as_u128().to_le_bytes());

        StoreIdentity {
            store_id,
            replica_id,
            creation_epoch: 1,
            schema_fingerprint,
        }
    }
}

/// Handle for a SQL database with built-in sync transport.
#[cfg(feature = "native")]
pub struct SqlDatabaseHandle {
    sdk: StorageSdk,
    info: DatabaseInfo,
    database: Arc<TokioMutex<Option<SqlDatabase>>>,
}

#[cfg(feature = "native")]
impl SqlDatabaseHandle {
    pub fn id(&self) -> DatabaseId {
        self.info.id
    }

    pub fn name(&self) -> &str {
        &self.info.name
    }

    pub fn application_id(&self) -> &str {
        &self.info.application_id
    }

    /// Get the underlying database (for direct SQL execution).
    pub async fn database(&self) -> StorageSdkResult<SqlDatabase> {
        let guard = self.database.lock().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| StorageSdkError::database("Database not initialized"))
    }

    /// Execute SQL directly on the local database.
    pub async fn execute(&self, sql: &str) -> StorageSdkResult<Vec<QueryResult>> {
        let db = self.database().await?;
        let client = db.client();
        let mut tx = client
            .transaction()
            .await
            .map_err(|e| StorageSdkError::database(e.to_string()))?;
        let result = tx
            .execute_sql(sql, None::<&QueryParams>)
            .await
            .map_err(|e| StorageSdkError::database(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| StorageSdkError::database(e.to_string()))?;
        Ok(result)
    }

    /// Synchronize with another device using the built-in transport.
    pub async fn synchronize(
        &self,
        _direction: ClientSyncDirection,
        role: SyncRole,
    ) -> StorageSdkResult<OfdbSyncResult> {
        let db = self.database().await?;
        let _store_identity = self.sdk.create_store_identity(self.info.id.into());

        let sync_url = format!("{}/sync/ws", self.sdk.http_config.lock().unwrap().base_path);
        let token = (self.sdk.token_provider)().await?;
        let ws_url = format!("{}?token={}", sync_url, token);

        let ws_transport = WebSocketSyncTransport::connect(&ws_url).await?;
        let mut transport = WsTransportBridge::new(ws_transport);

        let config = SessionConfig::default();

        let result = db
            .synchronize_checked(&mut transport, &config, role, || async { Ok(()) })
            .await
            .map_err(|e| StorageSdkError::database(e.to_string()))?;

        Ok(result)
    }

    /// Start a sync session to another device via the storage service.
    pub async fn sync_to_device(
        &self,
        target_device: DeviceId,
    ) -> StorageSdkResult<SyncSession> {
        self.sdk
            .sync_to_device(SyncToDeviceConfig {
                resource_id: self.info.id.into(),
                target_device,
            })
            .await
    }
}

/// Handle for a KV store with built-in sync transport.
#[cfg(feature = "native")]
pub struct KvStoreHandle {
    sdk: StorageSdk,
    info: KvStoreInfo,
    kv_store: Arc<TokioMutex<Option<ofdb_kv::Client>>>,
}

#[cfg(feature = "native")]
impl KvStoreHandle {
    pub fn id(&self) -> KvStoreId {
        self.info.id
    }

    pub fn name(&self) -> &str {
        &self.info.name
    }

    pub fn application_id(&self) -> &str {
        &self.info.application_id
    }

    pub async fn kv_store(&self) -> StorageSdkResult<ofdb_kv::Client> {
        let guard = self.kv_store.lock().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| StorageSdkError::kv_store("KV store not initialized"))
    }

    pub async fn get(&self, key: &[u8]) -> StorageSdkResult<Option<Vec<u8>>> {
        let kv = self.kv_store().await?;
        let key_str = String::from_utf8(key.to_vec())
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        let result = kv.get(&key_str)
            .await
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        Ok(result.map(|v| value_to_bytes(v)))
    }

    pub async fn set(&self, key: &[u8], value: &[u8]) -> StorageSdkResult<()> {
        let kv = self.kv_store().await?;
        let key_str = String::from_utf8(key.to_vec())
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        let value_str = String::from_utf8(value.to_vec())
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        let value = bytes_to_value(value_str.as_bytes());
        kv.set(key_str.as_str(), value, None::<i64>)
            .await
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))
    }

    pub async fn delete(&self, key: &[u8]) -> StorageSdkResult<()> {
        let kv = self.kv_store().await?;
        let key_str = String::from_utf8(key.to_vec())
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        kv.delete(key_str.as_str())
            .await
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))
    }

    pub async fn sync_to_device(
        &self,
        target_device: DeviceId,
    ) -> StorageSdkResult<SyncSession> {
        self.sdk
            .sync_to_device(SyncToDeviceConfig {
                resource_id: self.info.id.into(),
                target_device,
            })
            .await
    }
}

/// Handle for a filesystem with built-in sync transport.
#[cfg(feature = "native")]
pub struct FilesystemHandle {
    sdk: StorageSdk,
    info: FilesystemInfo,
    filesystem: Arc<TokioMutex<Option<file_system::FileSystem>>>,
}

#[cfg(feature = "native")]
impl FilesystemHandle {
    pub fn id(&self) -> FilesystemId {
        self.info.id
    }

    pub fn name(&self) -> &str {
        &self.info.name
    }

    pub fn application_id(&self) -> &str {
        &self.info.application_id
    }

    pub async fn filesystem(&self) -> StorageSdkResult<file_system::FileSystem> {
        let guard = self.filesystem.lock().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| StorageSdkError::filesystem("Filesystem not initialized"))
    }

    pub async fn sync_to_device(
        &self,
        target_device: DeviceId,
    ) -> StorageSdkResult<SyncSession> {
        self.sdk
            .sync_to_device(SyncToDeviceConfig {
                resource_id: self.info.id.into(),
                target_device,
            })
            .await
    }
}

/// Convert ofdb_value::Value to bytes.
#[cfg(feature = "native")]
fn value_to_bytes(value: OfdbValue) -> Vec<u8> {
    match value {
        OfdbValue::Blob(bytes) => bytes,
        OfdbValue::Text(s) => s.into_bytes(),
        OfdbValue::Integer(i) => i.to_string().into_bytes(),
        OfdbValue::Float(f) => f.to_string().into_bytes(),
        OfdbValue::Bool(b) => b.to_string().into_bytes(),
        OfdbValue::Uuid(u) => u.to_string().into_bytes(),
        OfdbValue::Null => Vec::new(),
        OfdbValue::Json(j) => serde_json::to_vec(&j).unwrap_or_default(),
        OfdbValue::Type(_) => Vec::new(),
    }
}

/// Convert bytes to ofdb_value::Value.
#[cfg(feature = "native")]
fn bytes_to_value(bytes: &[u8]) -> OfdbValue {
    // Try to parse as UTF-8 string first
    if let Ok(s) = String::from_utf8(bytes.to_vec()) {
        OfdbValue::Text(s)
    } else {
        OfdbValue::Blob(bytes.to_vec())
    }
}
