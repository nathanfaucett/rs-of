use alloc::string::String;
use serde::{Deserialize, Serialize};

pub type DatabaseId = crate::idp::model::Id;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DatabaseResource {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub id: DatabaseId,
    pub name: Option<String>,
}
