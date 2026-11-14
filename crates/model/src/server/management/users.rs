use alloc::string::String;

use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserApplicationRoleResponse {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub role_id: crate::idp::model::Id,
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub application_id: crate::idp::model::Id,
    pub role_name: String,
    pub role_description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<crate::idp::model::Role> for UserApplicationRoleResponse {
    fn from(value: crate::idp::model::Role) -> Self {
        Self {
            role_id: value.id,
            application_id: value.application_id,
            role_name: value.name,
            role_description: value.description,
            created_at: value.created_at.timestamp(),
            updated_at: value.updated_at.timestamp(),
        }
    }
}
