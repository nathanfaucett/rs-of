#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::Deserialize;
use serde::Serialize;

use crate::id::Id;
use crate::idp::model::role::Role;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RoleResponse {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub id: Id,
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub application_id: Id,
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

impl From<Role> for RoleResponse {
    fn from(role: Role) -> Self {
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
