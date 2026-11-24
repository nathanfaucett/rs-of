#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::{Deserialize, Serialize};

use crate::id::Id;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeviceEndpointIdentity {
    /// Device record ID, not an endpoint public key.
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub device_id: Id,
    /// User subject in the legacy Device owner binding, not the authenticated request actor or the endpoint identity.
    pub owner_subject: String,
    /// Canonical Iroh endpoint public key.
    pub endpoint_id: String,
}
