use alloc::string::String;

use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::storage::DatabaseId;

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateResourceRequest {
    pub name: Option<String>,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnsureDatabaseRequest {
    /// Case-sensitive application-chosen identity; 1–128 UTF-8 bytes.
    #[cfg_attr(feature = "utoipa", schema(min_length = 1, max_length = 128))]
    pub creation_key: String,
    pub name: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileSystemResourceResponse {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDetailResponse {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub id: DatabaseId,
    pub name: Option<String>,
    pub application_id: String,
}

#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileSystemDetailResponse {
    pub id: String,
    pub name: Option<String>,
    pub application_id: String,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncConnectRequest {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub resource_id: DatabaseId,
    pub target_device: Uuid,
    #[serde(default = "default_bidirectional")]
    pub _direction: SyncDirection,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum SyncDirection {
    Push,
    Pull,
    Bidirectional,
}

#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionResponse {
    pub session_id: Uuid,
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub resource_id: DatabaseId,
    pub source_device: Uuid,
    pub target_device: Uuid,
    pub status: SyncStatus,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum SyncStatus {
    Connecting,
    Syncing,
    Completed,
    Failed,
    Cancelled,
}

fn default_bidirectional() -> SyncDirection {
    SyncDirection::Bidirectional
}
