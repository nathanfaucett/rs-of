use alloc::string::String;
use alloc::vec::Vec;

use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum SqlValue {
    Null,
    Boolean(bool),
    Integer(String),
    Float(f64),
    Text(String),
    Uuid(String),
    Blob(Vec<u8>),
}

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct SqlStatement {
    /// SQL text, at most 16 KiB UTF-8; positional parameters only.
    pub sql: String,
    #[serde(default)]
    #[cfg_attr(feature = "utoipa", schema(max_items = 128))]
    pub params: Vec<SqlValue>,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct SqlRequest {
    #[cfg_attr(feature = "utoipa", schema(min_items = 1, max_items = 32))]
    pub statements: Vec<SqlStatement>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SqlResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<SqlValue>>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SqlResponse {
    pub results: Vec<SqlResult>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DatabaseFailure {
    pub code: &'static str,
}
