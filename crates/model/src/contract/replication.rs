use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

pub const MANAGEMENT_REPLICATION_READ_SCOPE: &str = "management.replication.read";
pub const MANAGEMENT_REPLICATION_ADMIT_SCOPE: &str = "management.replication.admit";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplicationAdmissionRequest {
    pub source_endpoint_id: String,
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
    pub owner_subject: String,
    pub application_id: String,
    pub kind: String,
    pub resource_id: String,
}

/// Current policy for an IdP-approved endpoint with an active owner.
/// Pending cleanup is service-only evidence, never replication approval or cacheable permission.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SelectedResourcesResponse {
    pub device_id: String,
    pub endpoint_id: String,
    pub owner_subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_cleanup: Option<OwnerCopyCleanup>,
    pub admin_allowed: bool,
    pub resources: Vec<SelectedResource>,
}

/// Exact current Management binding; only an authenticated Storage service may acknowledge it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnerCopyCleanup {
    pub cleanup_id: String,
    pub device_id: String,
    pub endpoint_id: String,
    pub old_owner_subject: String,
    pub new_owner_subject: String,
}
