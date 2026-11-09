//! Browser (WASM) storage client implementation.

#[cfg(feature = "browser")]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "browser")]
use core::future::Future;
#[cfg(feature = "browser")]
use core::pin::Pin;

#[cfg(feature = "browser")]
use js_sys::{Promise, Uint8Array};
#[cfg(feature = "browser")]
use wasm_bindgen::prelude::*;
#[cfg(feature = "browser")]
use wasm_bindgen_futures::JsFuture;
#[cfg(feature = "browser")]
use web_sys::{Request, RequestInit, RequestMode, Response};

#[cfg(feature = "browser")]
use storage_sync_client::browser::browser::BrowserSyncTransport;
#[cfg(feature = "browser")]
use storage_sync_client::browser::browser_client::{BrowserSyncClient, BrowserTokenProvider};
#[cfg(feature = "browser")]
use storage_sync_client::protocol::{StoreIdentity, SyncDirection};

#[cfg(feature = "browser")]
use crate::error::{Result, StorageClientError};
#[cfg(feature = "browser")]
use crate::handle::{SqlDatabaseHandle, KvStoreHandle, FilesystemHandle};
#[cfg(feature = "browser")]
use crate::transport::{BrowserWsTransport, TransportConfig};
#[cfg(feature = "browser")]
use crate::{ResourceId, DatabaseId, KvStoreId, FilesystemId, DeviceId, CreateDatabaseRequest, OpenDatabaseRequest, CreateKvStoreRequest, OpenKvStoreRequest, CreateFilesystemRequest, OpenFilesystemRequest, SyncToDeviceRequest, SyncToDeviceResponse, SyncSession, SyncStatus, DatabaseInfo, KvStoreInfo, FilesystemInfo};

#[cfg(feature = "browser")]
use ofdb_sql::{Database, SessionConfig, SyncResult as OfdbSyncResult, SyncRole, SyncTransport as OfdbSyncTransport};
#[cfg(feature = "browser")]
use storage_sync_client::client::WsTransportBridge;

#[cfg(feature = "browser")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[cfg(feature = "browser")]
macro_rules! console_log {
    ($($t:tt)*) => (log(&alloc::format!($($t)*)))
}

/// Browser storage client for WASM applications.
#[cfg(feature = "browser")]
pub struct StorageClient {
    base_url: String,
    idp_base_url: String,
    client_id: String,
    token_provider: BrowserTokenProvider,
    sync_ws_url: String,
    local_replica_id: uuid::Uuid,
}

#[cfg(feature = "browser")]
impl StorageClient {
    /// Create a new storage client.
    pub fn new(
        storage_base_url: &str,
        idp_base_url: &str,
        client_id: &str,
        token_provider: BrowserTokenProvider,
    ) -> Self {
        let mut sync_ws_url = storage_base_url.to_string();
        if sync_ws_url.starts_with("https://") {
            sync_ws_url = sync_ws_url.replace("https://", "wss://");
        } else if sync_ws_url.starts_with("http://") {
            sync_ws_url = sync_ws_url.replace("http://", "ws://");
        }
        sync_ws_url = format!("{}/sync/ws", sync_ws_url.trim_end_matches('/'));

        Self {
            base_url: storage_base_url.trim_end_matches('/').to_string(),
            idp_base_url: idp_base_url.trim_end_matches('/').to_string(),
            client_id: client_id.to_string(),
            token_provider,
            sync_ws_url,
            local_replica_id: uuid::Uuid::now_v7(),
        }
    }

    /// Get the sync WebSocket URL.
    pub fn sync_ws_url(&self) -> &str {
        &self.sync_ws_url
    }

    /// Get the local replica ID.
    pub fn local_replica_id(&self) -> uuid::Uuid {
        self.local_replica_id
    }

    /// Create a new SQL database.
    pub async fn create_sql_db(&self, name: Option<String>) -> Result<SqlDatabaseHandle> {
        let token = self.get_storage_token(&["write"]).await?;
        let request = serde_wasm_bindgen::to_value(&serde_json::json!({
            "createDatabaseRequest": {
                "name": name
            }
        })).map_err(|e| StorageClientError::Json(e.to_string()))?;

        let response = self
            .fetch_json(&format!("{}/storage/databases", self.base_url), "POST", Some(request), &token)
            .await?;

        let db_resource: DatabaseResource = serde_wasm_bindgen::from_value(response)
            .map_err(|e| StorageClientError::Json(e.to_string()))?;

        let store_identity = StoreIdentity {
            store_id: db_resource.id.0,
            replica_id: self.local_replica_id,
            creation_epoch: 0,
            schema_fingerprint: [0u8; 32],
        };

        let transport_config = TransportConfig::new(self.sync_ws_url.clone(), token);
        let token_provider = self.create_sync_token_provider();

        Ok(SqlDatabaseHandle::new(
            Database::open_indexeddb(&db_resource.id.0.to_string()).await
                .map_err(|e| StorageClientError::Database(e.to_string()))?,
            store_identity,
            transport_config,
            token_provider,
            self.local_replica_id,
        ))
    }

    /// Open an existing SQL database by ID.
    pub async fn open_sql_db(&self, id: DatabaseId) -> Result<SqlDatabaseHandle> {
        let token = self.get_storage_token(&["read"]).await?;

        let response = self
            .fetch_json(&format!("{}/storage/databases/{}", self.base_url, id.0), "GET", None, &token)
            .await?;

        let db_resource: DatabaseResource = serde_wasm_bindgen::from_value(response)
            .map_err(|e| StorageClientError::Json(e.to_string()))?;

        let store_identity = StoreIdentity {
            store_id: db_resource.id.0,
            replica_id: self.local_replica_id,
            creation_epoch: 0,
            schema_fingerprint: [0u8; 32],
        };

        let transport_config = TransportConfig::new(self.sync_ws_url.clone(), token);
        let token_provider = self.create_sync_token_provider();

        Ok(SqlDatabaseHandle::new(
            Database::open_indexeddb(&db_resource.id.0.to_string()).await
                .map_err(|e| StorageClientError::Database(e.to_string()))?,
            store_identity,
            transport_config,
            token_provider,
            self.local_replica_id,
        ))
    }

    /// List all SQL databases.
    pub async fn list_sql_dbs(&self) -> Result<Vec<DatabaseInfo>> {
        let token = self.get_storage_token(&["read"]).await?;

        let response = self
            .fetch_json(&format!("{}/storage/databases", self.base_url), "GET", None, &token)
            .await?;

        let dbs: Vec<DatabaseResource> = serde_wasm_bindgen::from_value(response)
            .map_err(|e| StorageClientError::Json(e.to_string()))?;

        Ok(dbs
            .into_iter()
            .map(|db| DatabaseInfo {
                id: db.id,
                name: db.name,
            })
            .collect())
    }

    /// Create a new KV store.
    pub async fn create_kv_store(&self, name: Option<String>) -> Result<KvStoreHandle> {
        let _ = name;
        let token = self.get_storage_token(&["write"]).await?;
        let store_identity = StoreIdentity {
            store_id: uuid::Uuid::now_v7(),
            replica_id: self.local_replica_id,
            creation_epoch: 0,
            schema_fingerprint: [0u8; 32],
        };

        let transport_config = TransportConfig::new(self.sync_ws_url.clone(), token);
        let token_provider = self.create_sync_token_provider();

        Ok(KvStoreHandle::new(
            store_identity,
            transport_config,
            token_provider,
            self.local_replica_id,
        ))
    }

    /// Open an existing KV store by ID.
    pub async fn open_kv_store(&self, id: KvStoreId) -> Result<KvStoreHandle> {
        let token = self.get_storage_token(&["read"]).await?;
        let transport_config = TransportConfig::new(self.sync_ws_url.clone(), token);
        let token_provider = self.create_sync_token_provider();

        Ok(KvStoreHandle::new(
            StoreIdentity {
                store_id: id.0,
                replica_id: self.local_replica_id,
                creation_epoch: 0,
                schema_fingerprint: [0u8; 32],
            },
            transport_config,
            token_provider,
            self.local_replica_id,
        ))
    }

    /// List all KV stores.
    pub async fn list_kv_stores(&self) -> Result<Vec<KvStoreInfo>> {
        Ok(Vec::new())
    }

    /// Create a new filesystem.
    pub async fn create_filesystem(&self, name: Option<String>) -> Result<FilesystemHandle> {
        let token = self.get_storage_token(&["write"]).await?;
        let request = serde_wasm_bindgen::to_value(&serde_json::json!({
            "createResourceRequest": {
                "name": name
            }
        })).map_err(|e| StorageClientError::Json(e.to_string()))?;

        let response = self
            .fetch_json(&format!("{}/storage/filesystems", self.base_url), "POST", Some(request), &token)
            .await?;

        let fs_resource: FilesystemResource = serde_wasm_bindgen::from_value(response)
            .map_err(|e| StorageClientError::Json(e.to_string()))?;

        let transport_config = TransportConfig::new(self.sync_ws_url.clone(), token);
        let token_provider = self.create_sync_token_provider();

        Ok(FilesystemHandle::new(
            StoreIdentity {
                store_id: fs_resource.id.0,
                replica_id: self.local_replica_id,
                creation_epoch: 0,
                schema_fingerprint: [0u8; 32],
            },
            transport_config,
            token_provider,
            self.local_replica_id,
        ))
    }

    /// Open an existing filesystem by ID.
    pub async fn open_filesystem(&self, id: FilesystemId) -> Result<FilesystemHandle> {
        let token = self.get_storage_token(&["read"]).await?;

        let response = self
            .fetch_json(&format!("{}/storage/filesystems/{}", self.base_url, id.0), "GET", None, &token)
            .await?;

        let fs_resource: FilesystemResource = serde_wasm_bindgen::from_value(response)
            .map_err(|e| StorageClientError::Json(e.to_string()))?;

        let transport_config = TransportConfig::new(self.sync_ws_url.clone(), token);
        let token_provider = self.create_sync_token_provider();

        Ok(FilesystemHandle::new(
            StoreIdentity {
                store_id: fs_resource.id.0,
                replica_id: self.local_replica_id,
                creation_epoch: 0,
                schema_fingerprint: [0u8; 32],
            },
            transport_config,
            token_provider,
            self.local_replica_id,
        ))
    }

    /// List all filesystems.
    pub async fn list_filesystems(&self) -> Result<Vec<FilesystemInfo>> {
        let token = self.get_storage_token(&["read"]).await?;

        let response = self
            .fetch_json(&format!("{}/storage/filesystems", self.base_url), "GET", None, &token)
            .await?;

        let fss: Vec<FilesystemResource> = serde_wasm_bindgen::from_value(response)
            .map_err(|e| StorageClientError::Json(e.to_string()))?;

        Ok(fss
            .into_iter()
            .map(|fs| FilesystemInfo {
                id: fs.id,
                name: fs.name,
            })
            .collect())
    }

    /// Start a sync session to a target device.
    pub async fn sync_to_device(
        &self,
        resource_id: ResourceId,
        target_device: DeviceId,
    ) -> Result<SyncSession> {
        let token = self.get_storage_token(&["write"]).await?;
        let request = serde_wasm_bindgen::to_value(&serde_json::json!({
            "resourceId": resource_id.0.to_string(),
            "targetDevice": target_device.0.to_string(),
        })).map_err(|e| StorageClientError::Json(e.to_string()))?;

        let response = self
            .fetch_json(&format!("{}/sync/connect", self.base_url), "POST", Some(request), &token)
            .await?;

        let sync_response: SyncToDeviceResponse = serde_wasm_bindgen::from_value(response)
            .map_err(|e| StorageClientError::Json(e.to_string()))?;

        Ok(sync_response.session)
    }

    async fn get_storage_token(&self, actions: &[&str]) -> Result<String> {
        let subject_token = (self.token_provider)().await
            .map_err(|e| StorageClientError::Auth(e.to_string()))?;

        let exchange_request = serde_wasm_bindgen::to_value(&serde_json::json!({
            "grantType": "urn:ietf:params:oauth:grant-type:token-exchange",
            "subjectToken": subject_token,
            "subjectTokenType": "urn:ietf:params:oauth:token-type:access_token",
            "resource": "storage:databases",
            "clientId": self.client_id,
            "authorizationDetails": serde_json::json!([{
                "type": "storage",
                "actions": actions
            }]),
        })).map_err(|e| StorageClientError::Json(e.to_string()))?;

        let response = self
            .fetch_json(&format!("{}/token", self.idp_base_url), "POST", Some(exchange_request), "")
            .await?;

        let token_response: TokenExchangeResponse = serde_wasm_bindgen::from_value(response)
            .map_err(|e| StorageClientError::Json(e.to_string()))?;

        token_response
            .access_token
            .ok_or_else(|| StorageClientError::Auth("No access token in response".into()))
    }

    fn create_sync_token_provider(&self) -> BrowserTokenProvider {
        let idp_base_url = self.idp_base_url.clone();
        let client_id = self.client_id.clone();
        let token_provider = self.token_provider.clone();

        Box::new(move || {
            let idp_base_url = idp_base_url.clone();
            let client_id = client_id.clone();
            let token_provider = token_provider.clone();

            Box::pin(async move {
                let subject_token = token_provider().await?;

                let exchange_request = serde_wasm_bindgen::to_value(&serde_json::json!({
                    "grantType": "urn:ietf:params:oauth:grant-type:token-exchange",
                    "subjectToken": subject_token,
                    "subjectTokenType": "urn:ietf:params:oauth:token-type:access_token",
                    "resource": "storage:sync",
                    "clientId": client_id,
                    "authorizationDetails": serde_json::json!([{
                        "type": "storage",
                        "actions": ["read", "write"]
                    }]),
                })).map_err(|e| StorageClientError::Json(e.to_string()))?;

                let response = fetch_json(&format!("{}/token", idp_base_url), "POST", Some(exchange_request), "")
                    .await?;

                let token_response: TokenExchangeResponse = serde_wasm_bindgen::from_value(response)
                    .map_err(|e| StorageClientError::Json(e.to_string()))?;

                token_response
                    .access_token
                    .ok_or_else(|| StorageClientError::Auth("No access token in response".into()))
            })
        })
    }

    async fn fetch_json(
        &self,
        url: &str,
        method: &str,
        body: Option<JsValue>,
        token: &str,
    ) -> Result<JsValue> {
        let mut init = RequestInit::new();
        init.method(method);
        init.mode(RequestMode::Cors);

        let mut headers = web_sys::Headers::new().map_err(|e| StorageClientError::Internal(format!("Headers error: {:?}", e)))?;
        headers.set("Content-Type", "application/json").map_err(|e| StorageClientError::Internal(format!("Headers error: {:?}", e)))?;
        if !token.is_empty() {
            headers.set("Authorization", &format!("Bearer {}", token)).map_err(|e| StorageClientError::Internal(format!("Headers error: {:?}", e)))?;
        }
        init.headers(&headers);

        if let Some(body) = body {
            init.body(Some(&body));
        }

        let request = Request::new_with_str_and_init(url, &init)
            .map_err(|e| StorageClientError::Internal(format!("Request error: {:?}", e)))?;

        let window = web_sys::window().ok_or_else(|| StorageClientError::Internal("No window".into()))?;
        let response_promise = window.fetch_with_request(&request);
        let response = JsFuture::from(response_promise)
            .await
            .map_err(|e| StorageClientError::Internal(format!("Fetch error: {:?}", e)))?;
        let response: Response = response.dyn_into().map_err(|e| StorageClientError::Internal(format!("Response error: {:?}", e)))?;

        if !response.ok() {
            return Err(StorageClientError::Internal(format!("HTTP error: {}", response.status())));
        }

        let json_promise = response.json()
            .map_err(|e| StorageClientError::Internal(format!("JSON error: {:?}", e)))?;
        let json = JsFuture::from(json_promise)
            .await
            .map_err(|e| StorageClientError::Internal(format!("JSON parse error: {:?}", e)))?;

        Ok(json)
    }
}

/// Database resource from REST API.
#[cfg(feature = "browser")]
#[derive(Debug, Clone, serde::Deserialize)]
struct DatabaseResource {
    #[serde(rename = "id")]
    id: ResourceId,
    name: Option<String>,
}

/// Filesystem resource from REST API.
#[cfg(feature = "browser")]
#[derive(Debug, Clone, serde::Deserialize)]
struct FilesystemResource {
    #[serde(rename = "id")]
    id: FilesystemId,
    name: Option<String>,
}

/// Token exchange response.
#[cfg(feature = "browser")]
#[derive(Debug, Clone, serde::Deserialize)]
struct TokenExchangeResponse {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "tokenType")]
    token_type: Option<String>,
    #[serde(rename = "expiresIn")]
    expires_in: Option<u64>,
}
