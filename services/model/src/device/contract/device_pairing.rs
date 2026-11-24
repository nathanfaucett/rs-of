#[cfg(not(feature = "std"))]
use alloc::{format, string::String};

use serde::{Deserialize, Serialize};

use crate::id::Id;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DevicePairingRequest {
    pub name: String,
    /// Canonical Iroh endpoint public key of the new Device.
    pub public_key: String,
    pub address: String,
    /// Canonical Iroh endpoint public key of the approved accepting Device.
    pub accepting_public_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DevicePairingApprovalRequest {
    /// Ed25519 signature over the exact UTF-8 approval payload, made with the new endpoint private key and encoded as base64url without padding.
    pub signature: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DevicePairingApprovalPayload {
    /// Exact text to sign as UTF-8 with the new endpoint private key.
    pub payload: String,
}

#[must_use]
pub fn device_pairing_approval_payload(
    device_id: Id,
    name: &str,
    public_key: &str,
    address: &str,
) -> String {
    format!(
        "idp-device-pairing-approval-v1\n{device_id}\n{}\n{}\n{}\n{}\n{}\n{}",
        name.len(),
        name,
        public_key.len(),
        public_key,
        address.len(),
        address,
    )
}
