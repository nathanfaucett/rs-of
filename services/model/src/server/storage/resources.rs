#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::Deserialize;
use serde::Serialize;

use crate::storage::database_resource::DatabaseId;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateResourceRequest {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnsureDatabaseRequest {
    /// Case-sensitive application-chosen identity; 1–128 UTF-8 bytes.
    #[cfg_attr(feature = "utoipa", schema(min_length = 1, max_length = 128))]
    pub creation_key: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileSystemResourceResponse {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDetailResponse {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub id: DatabaseId,
    pub name: Option<String>,
    pub application_id: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileSystemDetailResponse {
    pub id: String,
    pub name: Option<String>,
    pub application_id: String,
}
