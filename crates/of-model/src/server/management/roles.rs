use alloc::string::String;

use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RoleResponse {
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
pub struct ListRolesQuery {
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateRoleRequest {
    pub name: String,
    pub description: Option<String>,
}

impl From<crate::idp::model::Role> for RoleResponse {
    fn from(role: crate::idp::model::Role) -> Self {
        Self {
            id: role.id,
            application_id: role.application_id,
            name: role.name,
            description: role.description,
            created_at: role.created_at.timestamp(),
            updated_at: role.updated_at.timestamp(),
        }
    }
}

const fn default_limit() -> u32 {
    50
}
