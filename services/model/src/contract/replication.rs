#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

pub const MANAGEMENT_REPLICATION_READ_SCOPE: &str = "management.replication.read";
pub const MANAGEMENT_REPLICATION_ADMIT_SCOPE: &str = "management.replication.admit";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplicationAdmissionRequest {
    /// Canonical Iroh endpoint public key of the source Device.
    pub source_endpoint_id: String,
    /// Canonical Iroh endpoint public key of the target Device.
    pub target_endpoint_id: String,
    pub application_id: String,
    pub kind: String,
    pub resource_id: String,
    pub operation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SelectedResource {
    /// User subject in the Resource namespace, matched to the legacy Device owner binding. It is not the request actor or the endpoint identity.
    pub owner_subject: String,
    pub application_id: String,
    pub kind: String,
    pub resource_id: String,
}

/// Current policy for a Device-approved endpoint whose legacy owner is an active IdP User.
/// Pending cleanup is service-only evidence, never replication approval or cacheable permission.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SelectedResourcesResponse {
    /// Device record ID, not an endpoint public key.
    pub device_id: String,
    /// Canonical Iroh endpoint public key.
    pub endpoint_id: String,
    /// User subject in the legacy Device owner binding, not the authenticated request actor or the endpoint identity.
    pub owner_subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_cleanup: Option<OwnerCopyCleanup>,
    pub admin_allowed: bool,
    pub resources: Vec<SelectedResource>,
}

/// Exact cleanup binding for a Device owner transfer; only an authenticated Storage service may acknowledge it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnerCopyCleanup {
    pub cleanup_id: String,
    /// Device record ID, not an endpoint public key.
    pub device_id: String,
    /// Canonical Iroh endpoint public key.
    pub endpoint_id: String,
    /// Previous User subject in the legacy Device owner binding, not the request actor or the endpoint identity.
    pub old_owner_subject: String,
    /// New User subject in the legacy Device owner binding, not the request actor or the endpoint identity.
    pub new_owner_subject: String,
}
