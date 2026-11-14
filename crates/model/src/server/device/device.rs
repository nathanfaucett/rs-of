use alloc::string::String;

use serde::Serialize;

#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub public_key: String,
    pub address: String,
}
