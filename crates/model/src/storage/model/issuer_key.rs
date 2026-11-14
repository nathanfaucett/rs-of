use alloc::string::String;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuerKey {
    pub issuer: String,
    pub key_id: u32,
    pub public_key: String,
    pub revoked_at: Option<DateTime<Utc>>,
}
