use alloc::string::String;

use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PermissionResponse {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub id: crate::idp::model::Id,
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub application_id: crate::idp::model::Id,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams, utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", into_params(parameter_in = Query))]
pub struct ListPermissionsQuery {
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreatePermissionRequest {
    pub name: String,
    pub description: Option<String>,
}

impl From<crate::idp::model::Permission> for PermissionResponse {
    fn from(value: crate::idp::model::Permission) -> Self {
        Self {
            id: value.id,
            application_id: value.application_id,
            name: value.name,
            description: value.description,
            created_at: value.created_at.timestamp(),
            updated_at: value.updated_at.timestamp(),
        }
    }
}

const fn default_limit() -> u32 {
    50
}
