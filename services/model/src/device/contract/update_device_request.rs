#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::{Deserialize, Serialize};

use crate::id::Id;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeviceRequest {
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceTransferRequest {
    /// User subject to set in the legacy Device owner binding, not the authenticated request actor or the endpoint identity.
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub owner_subject: Id,
}
