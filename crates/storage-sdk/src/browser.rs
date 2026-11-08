#![allow(unused_imports)]

#[cfg(feature = "browser")]
use {
    alloc::{string::String, vec::Vec},
    ofdb_sql::{
        Database as SqlDatabase, SessionConfig, SyncResult as OfdbSyncResult, SyncRole, SyncTransport,
        Client as SqlClient, Transaction as SqlTransaction,
        query::{QueryParams, QueryResult},
    },
    ofdb_value::Value as OfdbValue,
    of_storage_client::apis::sync_api::sync_connect as http_sync_connect,
    of_storage_client::models::{SyncConnectRequest, SyncSessionResponse},
    storage_sync_client::{
        browser::browser::BrowserSyncTransport, protocol::StoreIdentity, TokenProvider,
    },
    std::sync::Arc,
    wasm_bindgen_futures::spawn_local,
    uuid::Uuid,
};

#[cfg(feature = "browser")]
use crate::{
    error::{StorageSdkError, StorageSdkResult},
    handle::{
        DatabaseId, DatabaseInfo, FilesystemId, FilesystemInfo, KvStoreId,
        KvStoreInfo,
    },
    sync::{DeviceId, ResourceId, SyncDirection as ClientSyncDirection, SyncResult, SyncSession, SyncToDeviceConfig},
};

/// Type alias for the token provider function.
#[cfg(feature = "browser")]
type BrowserTokenProvider = Box<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, storage_sync_client::SyncError>>>>>;

#[cfg(feature = "browser")]
#[derive(Clone)]
pub struct StorageSdk {
    base_url: String,
    token_provider: BrowserTokenProvider,
    application_id: String,
    user_sub: String,
    device_id: DeviceId,
}

#[cfg(feature = "browser")]
impl StorageSdk {
    /// Create a new storage SDK for browser/WASM.
    pub fn new(
        base_url: impl Into<String>,
        token_provider: BrowserTokenProvider,
        application_id: impl Into<String>,
        user_sub: impl Into<String>,
        device_id: DeviceId,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            token_provider,
            application_id: application_id.into(),
            user_sub: user_sub.into(),
            device_id,
        }
    }

    async fn http_post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> StorageSdkResult<T> {
        let url = format!("{}{}", self.base_url, path);
        let token = (self.token_provider)().await?;

        let mut opts = web_sys::RequestInit::new();
        opts.method("POST");
        opts.mode(web_sys::RequestMode::Cors);

        let headers = web_sys::Headers::new()
            .map_err(|_| StorageSdkError::other("Failed to create headers"))?;
        headers
            .set("Authorization", &format!("Bearer {}", token))
            .map_err(|_| StorageSdkError::other("Failed to set auth header"))?;
        headers
            .set("Content-Type", "application/json")
            .map_err(|_| StorageSdkError::other("Failed to set content type"))?;
        opts.headers(&headers);

        opts.body(Some(&wasm_bindgen::JsValue::from_str(&serde_json::to_string(body)?)));

        let request = web_sys::Request::new_with_str_and_init(&url, &opts)
            .map_err(|_| StorageSdkError::other("Failed to create request"))?;

        let window = web_sys::window().ok_or_else(|| StorageSdkError::other("No window"))?;
        let response_promise = window
            .fetch_with_request(&request)
            .map_err(|_| StorageSdkError::other("Fetch failed"))?;
        let response = wasm_bindgen_futures::JsFuture::from(response_promise)
            .await
            .map_err(|_| StorageSdkError::other("Fetch future failed"))?
            .dyn_into::<web_sys::Response>()
            .map_err(|_| StorageSdkError::other("Invalid response"))?;

        if !response.ok() {
            return Err(StorageSdkError::other(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let json_promise = response
            .json()
            .map_err(|_| StorageSdkError::other("Failed to parse JSON"))?;
        let json_value = wasm_bindgen_futures::JsFuture::from(json_promise)
            .await
            .map_err(|_| StorageSdkError::other("JSON future failed"))?;

        let result: T = serde_wasm_bindgen::from_value(json_value)
            .map_err(|e| StorageSdkError::Serialization(e.into()))?;

        Ok(result)
    }

    async fn http_get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> StorageSdkResult<T> {
        let url = format!("{}{}", self.base_url, path);
        let token = (self.token_provider)().await?;

        let mut opts = web_sys::RequestInit::new();
        opts.method("GET");
        opts.mode(web_sys::RequestMode::Cors);

        let headers = web_sys::Headers::new()
            .map_err(|_| StorageSdkError::other("Failed to create headers"))?;
        headers
            .set("Authorization", &format!("Bearer {}", token))
            .map_err(|_| StorageSdkError::other("Failed to set auth header"))?;
        opts.headers(&headers);

        let request = web_sys::Request::new_with_str_and_init(&url, &opts)
            .map_err(|_| StorageSdkError::other("Failed to create request"))?;

        let window = web_sys::window().ok_or_else(|| StorageSdkError::other("No window"))?;
        let response_promise = window
            .fetch_with_request(&request)
            .map_err(|_| StorageSdkError::other("Fetch failed"))?;
        let response = wasm_bindgen_futures::JsFuture::from(response_promise)
            .await
            .map_err(|_| StorageSdkError::other("Fetch future failed"))?
            .dyn_into::<web_sys::Response>()
            .map_err(|_| StorageSdkError::other("Invalid response"))?;

        if !response.ok() {
            return Err(StorageSdkError::other(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let json_promise = response
            .json()
            .map_err(|_| StorageSdkError::other("Failed to parse JSON"))?;
        let json_value = wasm_bindgen_futures::JsFuture::from(json_promise)
            .await
            .map_err(|_| StorageSdkError::other("JSON future failed"))?;

        let result: T = serde_wasm_bindgen::from_value(json_value)
            .map_err(|e| StorageSdkError::Serialization(e.into()))?;

        Ok(result)
    }

    /// Create a new SQL database.
    pub async fn create_sql_db(&self, name: &str) -> StorageSdkResult<SqlDatabaseHandle> {
        let request = serde_json::json!({ "name": name });
        let db_info: of_storage_client::models::DatabaseResource = self.http_post("/storage/databases", &request).await?;

        let database = self.open_local_database(&db_info.id).await?;

        Ok(SqlDatabaseHandle {
            sdk: self.clone(),
            info: DatabaseInfo {
                id: db_info.id,
                name: db_info.name.unwrap_or_default(),
                application_id: self.application_id.clone(),
            },
            database,
        })
    }

    /// Open an existing SQL database by ID.
    pub async fn open_sql_db(&self, id: DatabaseId) -> StorageSdkResult<SqlDatabaseHandle> {
        let db_info: of_storage_client::models::DatabaseDetailResponse = self.http_get(&format!("/storage/databases/{}", id)).await?;

        let database = self.open_local_database(&id).await?;

        Ok(SqlDatabaseHandle {
            sdk: self.clone(),
            info: DatabaseInfo {
                id: db_info.id,
                name: db_info.name.unwrap_or_default(),
                application_id: db_info.application_id,
            },
            database,
        })
    }

    /// List all SQL databases.
    pub async fn list_sql_dbs(&self) -> StorageSdkResult<Vec<DatabaseInfo>> {
        let databases: Vec<of_storage_client::models::DatabaseResource> = self.http_get("/storage/databases").await?;
        
        Ok(databases.into_iter().map(|d| DatabaseInfo {
            id: d.id,
            name: d.name.unwrap_or_default(),
            application_id: self.application_id.clone(),
        }).collect())
    }

    /// Create a new KV store.
    pub async fn create_kv_store(&self, name: &str) -> StorageSdkResult<KvStoreHandle> {
        let request = serde_json::json!({ "name": name });
        let info: of_storage_client::models::FileSystemResourceResponse = self.http_post("/storage/kv/create", &request).await?;

        let kv_store = self.open_local_kv_store(&info.id).await?;

        Ok(KvStoreHandle {
            sdk: self.clone(),
            info: KvStoreInfo {
                id: info.id,
                name: info.name.unwrap_or_default(),
                application_id: self.application_id.clone(),
            },
            kv_store,
        })
    }

    /// Open an existing KV store by ID.
    pub async fn open_kv_store(&self, id: KvStoreId) -> StorageSdkResult<KvStoreHandle> {
        let info: of_storage_client::models::FileSystemResourceResponse = self.http_get(&format!("/storage/kv/{}", id)).await?;

        let kv_store = self.open_local_kv_store(&id).await?;

        Ok(KvStoreHandle {
            sdk: self.clone(),
            info: KvStoreInfo {
                id: info.id,
                name: info.name.unwrap_or_default(),
                application_id: self.application_id.clone(),
            },
            kv_store,
        })
    }

    /// List all KV stores.
    pub async fn list_kv_stores(&self) -> StorageSdkResult<Vec<KvStoreInfo>> {
        let stores: Vec<of_storage_client::models::FileSystemResourceResponse> = self.http_get("/storage/kv").await?;
        
        Ok(stores.into_iter().map(|f| KvStoreInfo {
            id: f.id,
            name: f.name.unwrap_or_default(),
            application_id: self.application_id.clone(),
        }).collect())
    }

    /// Create a new filesystem.
    pub async fn create_filesystem(&self, name: &str) -> StorageSdkResult<FilesystemHandle> {
        let request = serde_json::json!({ "name": name });
        let info: of_storage_client::models::FileSystemResourceResponse = self.http_post("/storage/filesystems", &request).await?;

        let fs = self.open_local_filesystem(&info.id).await?;

        Ok(FilesystemHandle {
            sdk: self.clone(),
            info: FilesystemInfo {
                id: info.id,
                name: info.name.unwrap_or_default(),
                application_id: self.application_id.clone(),
            },
            filesystem: fs,
        })
    }

    /// Open an existing filesystem by ID.
    pub async fn open_filesystem(&self, id: FilesystemId) -> StorageSdkResult<FilesystemHandle> {
        let info: of_storage_client::models::FileSystemDetailResponse = self.http_get(&format!("/storage/filesystems/{}", id)).await?;

        let fs = self.open_local_filesystem(&id).await?;

        Ok(FilesystemHandle {
            sdk: self.clone(),
            info: FilesystemInfo {
                id: info.id,
                name: info.name.unwrap_or_default(),
                application_id: info.application_id,
            },
            filesystem: fs,
        })
    }

    /// List all filesystems.
    pub async fn list_filesystems(&self) -> StorageSdkResult<Vec<FilesystemInfo>> {
        let filesystems: Vec<of_storage_client::models::FileSystemResourceResponse> = self.http_get("/storage/filesystems").await?;
        
        Ok(filesystems.into_iter().map(|f| FilesystemInfo {
            id: f.id,
            name: f.name.unwrap_or_default(),
            application_id: self.application_id.clone(),
        }).collect())
    }

    /// Start a sync session to another device.
    pub async fn sync_to_device(
        &self,
        config: SyncToDeviceConfig,
    ) -> StorageSdkResult<SyncSession> {
        let request = SyncConnectRequest {
            resource_id: config.resource_id.to_string(),
            target_device: config.target_device.to_string(),
        };

        // Use the generated HTTP client
        let mut http_config = of_storage_client::apis::configuration::Configuration::new();
        http_config.base_path = self.base_url.clone();
        http_config.bearer_access_token = Some(((self.token_provider)().await?).into());

        let session = http_sync_connect(&http_config, of_storage_client::apis::sync_api::SyncConnectParams {
            sync_connect_request: request,
        }).await?;

        Ok(SyncSession {
            session_id: session.session_id,
            resource_id: ResourceId::from_uuid(Uuid::parse_str(&session.resource_id).unwrap()),
            source_device: DeviceId::from_uuid(Uuid::parse_str(&session.source_device).unwrap()),
            target_device: DeviceId::from_uuid(Uuid::parse_str(&session.target_device).unwrap()),
            status: match session.status.as_str() {
                "connecting" => SyncStatus::Connecting,
                "syncing" => SyncStatus::Syncing,
                "completed" => SyncStatus::Completed,
                "failed" => SyncStatus::Failed,
                "cancelled" => SyncStatus::Cancelled,
                _ => SyncStatus::Connecting,
            },
            bytes_sent: session.bytes_sent,
            bytes_received: session.bytes_received,
        })
    }

    async fn open_local_database(&self, id: &DatabaseId) -> StorageSdkResult<SqlDatabase> {
        let db_name = format!("ofdb-{}", id);
        let database = SqlDatabase::open_indexeddb(&db_name)
            .await
            .map_err(|e| StorageSdkError::database(e.to_string()))?;

        Ok(database)
    }

    async fn open_local_kv_store(&self, id: &KvStoreId) -> StorageSdkResult<ofdb_kv::KvStore> {
        let db_name = format!("ofkv-{}", id);
        let kv_store = ofdb_kv::KvStore::open_indexeddb(&db_name)
            .await
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;

        Ok(kv_store)
    }

    async fn open_local_filesystem(&self, id: &FilesystemId) -> StorageSdkResult<file_system::FileSystem> {
        let fs_name = format!("offs-{}", id);
        let fs = file_system::FileSystem::open(&fs_name)
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

/// Handle for a SQL database with built-in sync transport (browser/WASM).
#[cfg(feature = "browser")]
pub struct SqlDatabaseHandle {
    sdk: StorageSdk,
    info: DatabaseInfo,
    database: SqlDatabase,
}

#[cfg(feature = "browser")]
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
    pub fn database(&self) -> &SqlDatabase {
        &self.database
    }

    /// Execute SQL directly on the local database.
    pub async fn execute(&self, sql: &str) -> StorageSdkResult<Vec<QueryResult>> {
        let client = self.database.client();
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
        let sync_url = format!("{}/sync/ws", self.sdk.base_url);
        let token = (self.sdk.token_provider)().await?;

        let mut transport = BrowserSyncTransport::connect(&sync_url, &token)
            .await
            .map_err(|e| StorageSdkError::Sync(e.into()))?;

        let mut config = SessionConfig::default();
        config.direction = ofdb_sql::SyncDirection::Bidirectional;

        let result = self
            .database
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

/// Handle for a KV store with built-in sync transport (browser/WASM).
#[cfg(feature = "browser")]
pub struct KvStoreHandle {
    sdk: StorageSdk,
    info: KvStoreInfo,
    kv_store: ofdb_kv::KvStore,
}

#[cfg(feature = "browser")]
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

    pub fn kv_store(&self) -> &ofdb_kv::KvStore {
        &self.kv_store
    }

    pub async fn get(&self, key: &[u8]) -> StorageSdkResult<Option<Vec<u8>>> {
        let key_str = String::from_utf8(key.to_vec())
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        let result = self.kv_store.get(&key_str)
            .await
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        Ok(result.map(|v| v.as_bytes().to_vec()))
    }

    pub async fn set(&self, key: &[u8], value: &[u8]) -> StorageSdkResult<()> {
        let key_str = String::from_utf8(key.to_vec())
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        let value_str = String::from_utf8(value.to_vec())
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        let value = bytes_to_value(value_str.as_bytes());
        self.kv_store.set(&key_str, value, None::<i64>)
            .await
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))
    }

    pub async fn delete(&self, key: &[u8]) -> StorageSdkResult<()> {
        let key_str = String::from_utf8(key.to_vec())
            .map_err(|e| StorageSdkError::kv_store(e.to_string()))?;
        self.kv_store.delete(&key_str)
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

/// Handle for a filesystem with built-in sync transport (browser/WASM).
#[cfg(feature = "browser")]
pub struct FilesystemHandle {
    sdk: StorageSdk,
    info: FilesystemInfo,
    filesystem: file_system::FileSystem,
}

#[cfg(feature = "browser")]
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

    pub fn filesystem(&self) -> &file_system::FileSystem {
        &self.filesystem
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
#[cfg(feature = "browser")]
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
#[cfg(feature = "browser")]
fn bytes_to_value(bytes: &[u8]) -> OfdbValue {
    if let Ok(s) = String::from_utf8(bytes.to_vec()) {
        OfdbValue::Text(s)
    } else {
        OfdbValue::Blob(bytes.to_vec())
    }
}
