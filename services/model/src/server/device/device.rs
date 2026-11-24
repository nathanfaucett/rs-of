#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::Serialize;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Canonical Iroh endpoint public key.
    pub public_key: String,
    pub address: String,
}
