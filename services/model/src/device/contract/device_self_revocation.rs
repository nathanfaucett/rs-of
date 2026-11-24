#[cfg(not(feature = "std"))]
use alloc::{format, string::String};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeviceSelfRevocationRequest {
    /// Canonical Iroh endpoint public key of the Device to revoke.
    pub public_key: String,
    /// Ed25519 signature over the UTF-8 self-revocation payload, made with the endpoint private key and encoded as base64url without padding.
    pub signature: String,
}

#[must_use]
pub fn device_self_revocation_payload(public_key: &str) -> String {
    format!(
        "idp-device-self-revocation-v1\n{}\n{}",
        public_key.len(),
        public_key,
    )
}

#[cfg(test)]
mod tests {
    use crate::device::contract::device_self_revocation::device_self_revocation_payload;

    #[test]
    fn payload_is_length_delimited() {
        assert_eq!(
            device_self_revocation_payload("device-key"),
            "idp-device-self-revocation-v1\n10\ndevice-key"
        );
    }
}
