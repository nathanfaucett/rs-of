#[cfg(not(feature = "std"))]
use alloc::string::String;

use chrono::{DateTime, Utc};

use crate::id::Id;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Role {
    pub id: Id,

    pub application_id: Id,

    pub name: String,

    pub description: Option<String>,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
}
