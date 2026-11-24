use core::fmt;
#[cfg(not(target_arch = "wasm32"))]
use core::future::ready;

use alloc::{format, sync::Arc};

use ofdb_platform::{MaybeSend, MaybeSync};
use reqwest::Client as HttpClient;
use serde::{Deserialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{
    DatabaseId, DatabaseInfo, FilesystemId, FilesystemInfo, KvStoreId, KvStoreInfo, ResourceId,
    error::{Result, StorageClientError},
    handle::{FilesystemHandle, KvStoreHandle, SqlDatabaseHandle},
    protocol::StoreIdentity,
    transport::{TokenFuture, TokenProvider, TransportConfig},
};

#[cfg(not(target_arch = "wasm32"))]
type SubjectTokenProvider = Arc<dyn Fn(&str, &[&str]) -> TokenFuture + Send + Sync>;
#[cfg(target_arch = "wasm32")]
type SubjectTokenProvider = Arc<dyn Fn(&str, &[&str]) -> TokenFuture>;

#[derive(Clone)]
pub struct StorageClient {
    http_client: HttpClient,
    base_url: String,
    idp_base_url: String,
    client_id: String,
    token_provider: SubjectTokenProvider,
    sync_ws_url: String,
    local_replica_id: Uuid,
}

impl fmt::Debug for StorageClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorageClient")
            .field("base_url", &self.base_url)
            .field("idp_base_url", &self.idp_base_url)
            .field("client_id", &self.client_id)
            .field("sync_ws_url", &self.sync_ws_url)
            .field("local_replica_id", &self.local_replica_id)
            .finish_non_exhaustive()
    }
}
impl StorageClient {
    pub fn new(
        storage_base_url: &str,
        idp_base_url: &str,
        client_id: &str,
        token_provider: impl Fn(&str, &[&str]) -> TokenFuture + MaybeSend + MaybeSync + 'static,
    ) -> Self {
        let base_url = storage_base_url.trim_end_matches('/').to_string();
        let sync_ws_url = format!(
            "{}/sync/ws",
            base_url
                .replacen("https://", "wss://", 1)
                .replacen("http://", "ws://", 1)
        );
        Self {
            http_client: HttpClient::new(),
            base_url,
            idp_base_url: idp_base_url.trim_end_matches('/').to_string(),
            client_id: client_id.to_string(),
            token_provider: Arc::new(token_provider),
            sync_ws_url,
            local_replica_id: Uuid::now_v7(),
        }
    }
    #[must_use]
    pub fn sync_ws_url(&self) -> &str {
        &self.sync_ws_url
    }
    #[must_use]
    pub const fn local_replica_id(&self) -> Uuid {
        self.local_replica_id
    }

    /// Creates and opens a SQL database Resource.
    ///
    /// # Errors
    /// Returns an error if authentication, resource creation, response decoding, or local
    /// database opening fails.
    pub async fn create_sql_db(&self, name: Option<String>) -> Result<SqlDatabaseHandle> {
        let resource: Resource = self.create_resource("databases", name).await?;
        self.sql_handle(resource.id).await
    }
    /// Opens a SQL database Resource.
    ///
    /// # Errors
    /// Returns an error if authentication or the request fails, the Resource is missing, the
    /// response is invalid, or local database opening fails.
    pub async fn open_sql_db(&self, id: DatabaseId) -> Result<SqlDatabaseHandle> {
        let resource: Resource = self.open_resource("databases", id).await?;
        self.sql_handle(resource.id).await
    }
    /// Lists SQL database Resources.
    ///
    /// # Errors
    /// Returns an error if authentication, the request, or response decoding fails.
    pub async fn list_sql_dbs(&self) -> Result<Vec<DatabaseInfo>> {
        self.list_resources("databases").await
    }
    /// Creates and opens a KV store Resource.
    ///
    /// # Errors
    /// Returns an error if authentication, resource creation, response decoding, or local store
    /// opening fails.
    pub async fn create_kv_store(&self, name: Option<String>) -> Result<KvStoreHandle> {
        let resource: Resource = self.create_resource("databases", name).await?;
        self.kv_handle(resource.id).await
    }
    /// Opens a KV store Resource.
    ///
    /// # Errors
    /// Returns an error if authentication or the request fails, the Resource is missing, the
    /// response is invalid, or local store opening fails.
    pub async fn open_kv_store(&self, id: KvStoreId) -> Result<KvStoreHandle> {
        let resource: Resource = self.open_resource("databases", id).await?;
        self.kv_handle(resource.id).await
    }
    /// Lists KV store Resources.
    ///
    /// # Errors
    /// Returns an error if authentication, the request, or response decoding fails.
    pub async fn list_kv_stores(&self) -> Result<Vec<KvStoreInfo>> {
        let resources: Vec<Resource> = self.list_resources("databases").await?;
        Ok(resources
            .into_iter()
            .map(|r| KvStoreInfo {
                id: r.id,
                name: r.name,
            })
            .collect())
    }
    /// Creates a filesystem Resource.
    ///
    /// # Errors
    /// Returns an error if authentication, resource creation, or response decoding fails.
    pub async fn create_filesystem(&self, name: Option<String>) -> Result<FilesystemHandle> {
        let resource: Resource = self.create_resource("filesystems", name).await?;
        Ok(FilesystemHandle::new(self.identity(resource.id)))
    }
    /// Opens a filesystem Resource.
    ///
    /// # Errors
    /// Returns an error if authentication or the request fails, the Resource is missing, or the
    /// response is invalid.
    pub async fn open_filesystem(&self, id: FilesystemId) -> Result<FilesystemHandle> {
        let resource: Resource = self.open_resource("filesystems", id).await?;
        Ok(FilesystemHandle::new(self.identity(resource.id)))
    }
    /// Lists filesystem Resources.
    ///
    /// # Errors
    /// Returns an error if authentication, the request, or response decoding fails.
    pub async fn list_filesystems(&self) -> Result<Vec<FilesystemInfo>> {
        self.list_resources("filesystems").await
    }
    async fn create_resource<T: DeserializeOwned>(
        &self,
        kind: &str,
        name: Option<String>,
    ) -> Result<T> {
        let token = self
            .get_storage_token(&format!("storage:{kind}"), &["write"])
            .await?;
        Ok(self
            .http_client
            .post(format!("{}/storage/{kind}", self.base_url))
            .bearer_auth(token)
            .json(&serde_json::json!({"name": name}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
    async fn open_resource<T: DeserializeOwned>(&self, kind: &str, id: ResourceId) -> Result<T> {
        let token = self
            .get_storage_token(&format!("storage:{kind}"), &["read"])
            .await?;
        let response = self
            .http_client
            .get(format!("{}/storage/{kind}/{id}", self.base_url))
            .bearer_auth(token)
            .send()
            .await?;
        if response.status() == 404 {
            return Err(StorageClientError::NotFound(id.to_string()));
        }
        Ok(response.error_for_status()?.json().await?)
    }
    async fn list_resources<T: DeserializeOwned>(&self, kind: &str) -> Result<T> {
        let token = self
            .get_storage_token(&format!("storage:{kind}"), &["read"])
            .await?;
        Ok(self
            .http_client
            .get(format!("{}/storage/{kind}", self.base_url))
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
    const fn identity(&self, id: ResourceId) -> StoreIdentity {
        StoreIdentity {
            store_id: id.0,
            replica_id: self.local_replica_id,
            creation_epoch: 0,
            schema_fingerprint: [0; 32],
        }
    }
    fn sql_handle(&self, id: ResourceId) -> impl Future<Output = Result<SqlDatabaseHandle>> {
        #[cfg(not(target_arch = "wasm32"))]
        let database = ready(
            ofdb_sql::Database::open(id.to_string())
                .map_err(|e| StorageClientError::Database(e.to_string())),
        );
        #[cfg(target_arch = "wasm32")]
        let database = async move {
            ofdb_sql::Database::open_indexeddb(&id.to_string())
                .await
                .map_err(|e| StorageClientError::Database(e.to_string()))
        };
        async move {
            let database = database.await?;
            Ok(SqlDatabaseHandle::new(
                database,
                self.identity(id),
                TransportConfig::new(self.sync_ws_url.clone()),
                self.create_sync_token_provider(),
            ))
        }
    }
    fn kv_handle(&self, id: ResourceId) -> impl Future<Output = Result<KvStoreHandle>> {
        #[cfg(not(target_arch = "wasm32"))]
        let database = ready(
            ofdb_kv::Database::open(format!("{id}.kv"))
                .map_err(|e| StorageClientError::KvStore(e.to_string())),
        );
        #[cfg(target_arch = "wasm32")]
        let database = async move {
            ofdb_kv::Database::open_indexeddb(&format!("{id}.kv"))
                .await
                .map_err(|e| StorageClientError::KvStore(e.to_string()))
        };
        async move {
            let database = database.await?;
            Ok(KvStoreHandle::new(database, self.identity(id)))
        }
    }
    /// Exchanges a subject token for a Storage access token.
    ///
    /// # Errors
    /// Returns an error if the subject-token provider or token request fails, the response is
    /// invalid, or the returned access token is empty.
    pub async fn get_storage_token(&self, resource: &str, actions: &[&str]) -> Result<String> {
        let subject_token = (self.token_provider)(resource, actions).await?;
        let details = serde_json::json!([{"type": "storage", "actions": actions}]).to_string();
        let response = self
            .http_client
            .post(format!("{}/oauth2/token", self.idp_base_url))
            .form(&[
                (
                    "grant_type",
                    "urn:ietf:params:oauth:grant-type:token-exchange",
                ),
                ("subject_token", subject_token.as_str()),
                (
                    "subject_token_type",
                    "urn:ietf:params:oauth:token-type:access_token",
                ),
                ("resource", resource),
                ("client_id", self.client_id.as_str()),
                ("authorization_details", details.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?;
        let response: TokenResponse = response.json().await?;
        if response.access_token.is_empty() {
            return Err(StorageClientError::Auth("empty access token".into()));
        }
        Ok(response.access_token)
    }
    fn create_sync_token_provider(&self) -> TokenProvider {
        let client = self.clone();
        Arc::new(move || {
            let client = client.clone();
            Box::pin(async move {
                client
                    .get_storage_token("storage:sync", &["read", "write"])
                    .await
            })
        })
    }
}

#[derive(Deserialize)]
struct Resource {
    id: ResourceId,
    name: Option<String>,
}
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}
