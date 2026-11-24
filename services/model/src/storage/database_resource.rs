#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::{Deserialize, Serialize};

use crate::id::Id;

pub type DatabaseId = Id;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DatabaseResource {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub id: DatabaseId,
    pub name: Option<String>,
}
