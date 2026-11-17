//! Storage Client errors.

use alloc::string::{String, ToString};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageClientError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Sync error: {0}")]
    Sync(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("KV store error: {0}")]
    KvStore(String),

    #[error("Filesystem error: {0}")]
    Filesystem(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = core::result::Result<T, StorageClientError>;

impl From<StorageClientError> for String {
    fn from(e: StorageClientError) -> Self {
        e.to_string()
    }
}
