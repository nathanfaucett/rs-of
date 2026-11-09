//! Native (Rust) storage client implementation.

use alloc::{string::String, sync::Arc, vec::Vec};
use core::future::Future;
use core::pin::Pin;

use reqwest::Client as HttpClient;
use storage_sync_client::client::TokenProvider;
use storage_sync_client::protocol::StoreIdentity;
use uuid::Uuid;

use crate::error::{Result, StorageClientError};
use crate::handle::{SqlDatabaseHandle, KvStoreHandle, FilesystemHandle};
use crate::transport::TransportConfig;
use crate::{ResourceId, DatabaseId, KvStoreId, FilesystemId, DeviceId, SyncToDeviceRequest, SyncToDeviceResponse, SyncSession, DatabaseInfo, KvStoreInfo, FilesystemInfo};

use ofdb_sql::Database;

/// Native storage client for Rust applications.
pub struct StorageClient {
    http_client: HttpClient,
    base_url: String,
    idp_base_url: String,
    client_id: String,
    token_provider: Arc<dyn Fn(&str, &[&str]) -> Pin<Box<dyn Future<Output = core::result::Result<String, StorageClientError>> + Send>> + Send + Sync>,
    sync_ws_url: String,
    local_replica_id: Uuid,
}

impl StorageClient {
    /// Create a new storage client.
    pub fn new(
        storage_base_url: &str,
        idp_base_url: &str,
        client_id: &str,
        token_provider: impl Fn(&str, &[&str]) -> Pin<Box<dyn Future<Output = core::result::Result<String, StorageClientError>> + Send>> + Send + Sync + 'static,
    ) -> Self {
        let token_provider = Arc::new(token_provider);
        let mut sync_ws_url = storage_base_url.to_string();
        if sync_ws_url.starts_with("https://") {
            sync_ws_url = sync_ws_url.replace("https://", "wss://");
        } else if sync_ws_url.starts_with("http://") {
            sync_ws_url = sync_ws_url.replace("http://", "ws://");
        }
        sync_ws_url = format!("{}/sync/ws", sync_ws_url.trim_end_matches('/'));

        Self {
            http_client: HttpClient::new(),
            base_url: storage_base_url.trim_end_matches('/').to_string(),
            idp_base_url: idp_base_url.trim_end_matches('/').to_string(),
            client_id: client_id.to_string(),
            token_provider,
            sync_ws_url,
            local_replica_id: Uuid::now_v7(),
        }
    }

    /// Get the sync WebSocket URL.
    pub fn sync_ws_url(&self) -> &str {
        &self.sync_ws_url
    }

    /// Get the local replica ID.
    pub fn local_replica_id(&self) -> Uuid {
        self.local_replica_id
    }

    /// Create a new SQL database.
    pub async fn create_sql_db(&self, name: Option<String>) -> Result<SqlDatabaseHandle> {
        let token = self.get_storage_token(&["write"]).await?;
        let request = serde_json::json!({
            "createDatabaseRequest": {
                "name": name
            }
        });

        let response = self
            .http_client
            .post(&format!("{}/storage/databases", self.base_url))
            .bearer_auth(&token)
            .json(&request)
            .send()
            .await
            .map_err(StorageClientError::Http)?;

        let status = response.status();
        if !status.is_success() {
            return Err(StorageClientError::Http(
                response.error_for_status().unwrap_err(),
            ));
        }

        let db_resource: DatabaseResource = response.json().await.map_err(|e| StorageClientError::Http(e))?;

        let store_identity = StoreIdentity {
            store_id: db_resource.id.0,
            replica_id: self.local_replica_id,
            creation_epoch: 0,
            schema_fingerprint: [0u8; 32],
        };

        let transport_config = TransportConfig::new(self.sync_ws_url.clone(), token);
        let token_provider = self.create_sync_token_provider();

        Ok(SqlDatabaseHandle::new(
            Database::open_existing(&db_resource.id.0.to_string())
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
            .http_client
            .get(&format!("{}/storage/databases/{}", self.base_url, id.0))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(StorageClientError::Http)?;

        let status = response.status();
        if status == 404 {
            return Err(StorageClientError::NotFound(format!("Database not found: {}", id)));
        }
        if !status.is_success() {
            return Err(StorageClientError::Http(
                response.error_for_status().unwrap_err(),
            ));
        }

        let db_resource: DatabaseResource = response.json().await.map_err(|e| StorageClientError::Http(e))?;

        let store_identity = StoreIdentity {
            store_id: db_resource.id.0,
            replica_id: self.local_replica_id,
            creation_epoch: 0,
            schema_fingerprint: [0u8; 32],
        };

        let transport_config = TransportConfig::new(self.sync_ws_url.clone(), token);
        let token_provider = self.create_sync_token_provider();

        Ok(SqlDatabaseHandle::new(
            Database::open_existing(&db_resource.id.0.to_string())
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
            .http_client
            .get(&format!("{}/storage/databases", self.base_url))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(StorageClientError::Http)?;

        let status = response.status();
        if !status.is_success() {
            return Err(StorageClientError::Http(
                response.error_for_status().unwrap_err(),
            ));
        }

        let dbs: Vec<DatabaseResource> = response.json().await.map_err(|e| StorageClientError::Http(e))?;

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
        // TODO: Implement when KV REST API is available
        let _ = name;
        let token = self.get_storage_token(&["write"]).await?;
        let store_identity = StoreIdentity {
            store_id: Uuid::now_v7(),
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
        // TODO: Implement when KV REST API is available
        Ok(Vec::new())
    }

    /// Create a new filesystem.
    pub async fn create_filesystem(&self, name: Option<String>) -> Result<FilesystemHandle> {
        let token = self.get_storage_token(&["write"]).await?;
        let request = serde_json::json!({
            "createResourceRequest": {
                "name": name
            }
        });

        let response = self
            .http_client
            .post(&format!("{}/storage/filesystems", self.base_url))
            .bearer_auth(&token)
            .json(&request)
            .send()
            .await
            .map_err(StorageClientError::Http)?;

        let status = response.status();
        if !status.is_success() {
            return Err(StorageClientError::Http(
                response.error_for_status().unwrap_err(),
            ));
        }

        let fs_resource: FilesystemResource = response.json().await.map_err(|e| StorageClientError::Http(e))?;

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
            .http_client
            .get(&format!("{}/storage/filesystems/{}", self.base_url, id.0))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(StorageClientError::Http)?;

        let status = response.status();
        if status == 404 {
            return Err(StorageClientError::NotFound(format!("Filesystem not found: {}", id)));
        }
        if !status.is_success() {
            return Err(StorageClientError::Http(
                response.error_for_status().unwrap_err(),
            ));
        }

        let fs_resource: FilesystemResource = response.json().await.map_err(|e| StorageClientError::Http(e))?;

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
            .http_client
            .get(&format!("{}/storage/filesystems", self.base_url))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(StorageClientError::Http)?;

        let status = response.status();
        if !status.is_success() {
            return Err(StorageClientError::Http(
                response.error_for_status().unwrap_err(),
            ));
        }

        let fss: Vec<FilesystemResource> = response.json().await.map_err(|e| StorageClientError::Http(e))?;

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
        let request = SyncToDeviceRequest {
            resource_id,
            target_device,
        };

        let response = self
            .http_client
            .post(&format!("{}/sync/connect", self.base_url))
            .bearer_auth(&token)
            .json(&request)
            .send()
            .await
            .map_err(StorageClientError::Http)?;

        let status = response.status();
        if !status.is_success() {
            return Err(StorageClientError::Http(
                response.error_for_status().unwrap_err(),
            ));
        }

        let sync_response: SyncToDeviceResponse = response.json().await.map_err(|e| StorageClientError::Http(e))?;
        Ok(sync_response.session)
    }

    async fn get_storage_token(&self, actions: &[&str]) -> Result<String> {
        // Exchange IdP token for storage token
        let subject_token = (self.token_provider)("storage", actions).await?;

        let exchange_request = serde_json::json!({
            "grantType": "urn:ietf:params:oauth:grant-type:token-exchange",
            "subjectToken": subject_token,
            "subjectTokenType": "urn:ietf:params:oauth:token-type:access_token",
            "resource": "storage:databases",
            "clientId": self.client_id,
            "authorizationDetails": serde_json::json!([{
                "type": "storage",
                "actions": actions
            }]),
        });

        let response = self
            .http_client
            .post(&format!("{}/token", self.idp_base_url))
            .json(&exchange_request)
            .send()
            .await
            .map_err(StorageClientError::Http)?;

        let status = response.status();
        if !status.is_success() {
            return Err(StorageClientError::Http(
                response.error_for_status().unwrap_err(),
            ));
        }

        let token_response: TokenExchangeResponse = response.json().await.map_err(|e| StorageClientError::Http(e))?;

        token_response
            .access_token
            .ok_or_else(|| StorageClientError::Auth("No access token in response".into()))
    }

    fn create_sync_token_provider(&self) -> TokenProvider {
        let http_client = self.http_client.clone();
        let idp_base_url = self.idp_base_url.clone();
        let client_id = self.client_id.clone();
        let token_provider = self.token_provider.clone();

        Box::new(move || {
            let http_client = http_client.clone();
            let idp_base_url = idp_base_url.clone();
            let client_id = client_id.clone();
            let token_provider = token_provider.clone();

            Box::pin(async move {
                let subject_token = token_provider("storage", &["read", "write"]).await
                    .map_err(|e| storage_sync_client::SyncError::Auth(e.to_string()))?;

                let exchange_request = serde_json::json!({
                    "grantType": "urn:ietf:params:oauth:grant-type:token-exchange",
                    "subjectToken": subject_token,
                    "subjectTokenType": "urn:ietf:params:oauth:token-type:access_token",
                    "resource": "storage:sync",
                    "clientId": client_id,
                    "authorizationDetails": serde_json::json!([{
                        "type": "storage",
                        "actions": ["read", "write"]
                    }]),
                });

                let response = http_client
                    .post(&format!("{}/token", idp_base_url))
                    .json(&exchange_request)
                    .send()
                    .await
                    .map_err(|e| storage_sync_client::SyncError::Auth(e.to_string()))?;

                let status = response.status();
                if !status.is_success() {
                    return Err(storage_sync_client::SyncError::Auth(
                        format!("Token exchange failed: {}", status),
                    ));
                }

                let token_response: TokenExchangeResponse = response.json().await
                    .map_err(|e| storage_sync_client::SyncError::Auth(e.to_string()))?;

                token_response
                    .access_token
                    .ok_or_else(|| storage_sync_client::SyncError::Auth("No access token in response".into()))
            })
        })
    }
}

/// Database resource from REST API.
#[derive(Debug, Clone, serde::Deserialize)]
struct DatabaseResource {
    #[serde(rename = "id")]
    id: ResourceId,
    name: Option<String>,
}

/// Filesystem resource from REST API.
#[derive(Debug, Clone, serde::Deserialize)]
struct FilesystemResource {
    #[serde(rename = "id")]
    id: FilesystemId,
    name: Option<String>,
}

/// Token exchange response.
#[derive(Debug, Clone, serde::Deserialize)]
struct TokenExchangeResponse {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "tokenType")]
    _token_type: Option<String>,
    #[serde(rename = "expiresIn")]
    _expires_in: Option<u64>,
}
