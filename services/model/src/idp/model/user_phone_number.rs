#[cfg(not(feature = "std"))]
use alloc::string::String;

use chrono::{DateTime, Utc};

use crate::id::Id;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserPhoneNumber {
    pub id: Id,

    pub user_id: Id,

    /// E.164 format.
    pub phone_number: String,

    pub verified: bool,

    /// Only one phone number per user should be primary.
    pub primary: bool,

    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub updated_at: DateTime<Utc>,
}
