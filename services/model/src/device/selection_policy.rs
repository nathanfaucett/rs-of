use crate::id::Id as Uuid;

#[cfg(feature = "ofdb")]
use ofdb_sql::FromRow;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ofdb", derive(FromRow))]
pub struct SelectionPolicy {
    pub device_id: Uuid,
    pub owner_subject: String,
    pub application_id: Option<Uuid>,
    pub selected_kind: Option<String>,
    pub selected_id: Option<Uuid>,
    pub admin_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedResource {
    pub device_id: Uuid,
    pub owner_subject: String,
    pub application_id: Uuid,
    pub kind: String,
    pub resource_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerPolicyObservation {
    MissingPolicy,
    Ready {
        policy: SelectionPolicy,
        resources: Vec<SelectedResource>,
    },
    PendingCleanup {
        cleanup_id: Uuid,
        old_owner_subject: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "ofdb", derive(FromRow))]
pub struct ResourceSelection {
    pub id: Uuid,
    pub device_id: Uuid,
    pub owner_subject: String,
    pub application_id: Uuid,
    pub selected_kind: String,
    pub selected_id: Uuid,
    pub selected: bool,
}
