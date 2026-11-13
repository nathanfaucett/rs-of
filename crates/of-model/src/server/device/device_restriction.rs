use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceRestriction {
    pub admin_allowed: bool,
}
