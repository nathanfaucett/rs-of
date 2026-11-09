use thiserror::Error;

/// Result type for storage SDK operations.
pub type StorageSdkResult<T> = Result<T, StorageSdkError>;

/// Errors that can occur in the storage SDK.
#[derive(Debug, Error)]
pub enum StorageSdkError {
    #[error("HTTP request failed: {0}")]
    Http(Box<dyn std::error::Error + Send + Sync>),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),

    #[cfg(feature = "native")]
    #[error("Sync error: {0}")]
    Sync(#[from] storage_sync_client::SyncError),

    #[cfg(feature = "native")]
    #[error("Transport error: {0}")]
    Transport(#[from] storage_sync_client::transport::TransportError),

    #[cfg(feature = "browser")]
    #[error("Sync error: {0}")]
    Sync(#[from] storage_sync_client::SyncError),

    #[cfg(feature = "browser")]
    #[error("Transport error: {0}")]
    Transport(#[from] storage_sync_client::browser::browser::TransportError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("KV store error: {0}")]
    KvStore(String),

    #[error("Filesystem error: {0}")]
    Filesystem(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(String),
}

impl StorageSdkError {
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    pub fn kv_store(msg: impl Into<String>) -> Self {
        Self::KvStore(msg.into())
    }

    pub fn filesystem(msg: impl Into<String>) -> Self {
        Self::Filesystem(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }

    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

// Implement From for various HTTP error types
impl From<of_storage_api_client::apis::Error<of_storage_api_client::apis::sync_api::SyncConnectError>> for StorageSdkError {
    fn from(e: of_storage_api_client::apis::Error<of_storage_api_client::apis::sync_api::SyncConnectError>) -> Self {
        Self::Http(Box::new(e))
    }
}

impl From<of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::CreateDatabaseError>> for StorageSdkError {
    fn from(e: of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::CreateDatabaseError>) -> Self {
        Self::Http(Box::new(e))
    }
}

impl From<of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::GetDatabaseError>> for StorageSdkError {
    fn from(e: of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::GetDatabaseError>) -> Self {
        Self::Http(Box::new(e))
    }
}

impl From<of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::ListDatabasesError>> for StorageSdkError {
    fn from(e: of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::ListDatabasesError>) -> Self {
        Self::Http(Box::new(e))
    }
}

impl From<of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::DeleteDatabaseError>> for StorageSdkError {
    fn from(e: of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::DeleteDatabaseError>) -> Self {
        Self::Http(Box::new(e))
    }
}

impl From<of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::CreateFileSystemError>> for StorageSdkError {
    fn from(e: of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::CreateFileSystemError>) -> Self {
        Self::Http(Box::new(e))
    }
}

impl From<of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::GetFileSystemError>> for StorageSdkError {
    fn from(e: of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::GetFileSystemError>) -> Self {
        Self::Http(Box::new(e))
    }
}

impl From<of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::ListFileSystemsError>> for StorageSdkError {
    fn from(e: of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::ListFileSystemsError>) -> Self {
        Self::Http(Box::new(e))
    }
}

impl From<of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::DeleteFileSystemError>> for StorageSdkError {
    fn from(e: of_storage_api_client::apis::Error<of_storage_api_client::apis::resources_api::DeleteFileSystemError>) -> Self {
        Self::Http(Box::new(e))
    }
}
