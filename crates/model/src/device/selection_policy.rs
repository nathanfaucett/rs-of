use crate::id::Id as Uuid;
use alloc::{string::String, vec::Vec};

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[cfg(feature = "replica")]
use ofdb_sql::{FromRow, FromRowError, Row};

#[cfg(feature = "replica")]
impl FromRow for SelectionPolicy {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        Ok(Self {
            device_id: ofdb_sql::decode(ofdb_sql::value(row, columns, "device_id")?, "device_id")?,
            owner_subject: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "owner_subject")?,
                "owner_subject",
            )?,
            application_id: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "application_id")?,
                "application_id",
            )?,
            selected_kind: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "selected_kind")?,
                "selected_kind",
            )?,
            selected_id: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "selected_id")?,
                "selected_id",
            )?,
            admin_allowed: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "admin_allowed")?,
                "admin_allowed",
            )?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceSelection {
    pub id: Uuid,
    pub device_id: Uuid,
    pub owner_subject: String,
    pub application_id: Uuid,
    pub selected_kind: String,
    pub selected_id: Uuid,
    pub selected: bool,
}

#[cfg(feature = "replica")]
impl FromRow for ResourceSelection {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        Ok(Self {
            id: ofdb_sql::decode(ofdb_sql::value(row, columns, "id")?, "id")?,
            device_id: ofdb_sql::decode(ofdb_sql::value(row, columns, "device_id")?, "device_id")?,
            owner_subject: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "owner_subject")?,
                "owner_subject",
            )?,
            application_id: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "application_id")?,
                "application_id",
            )?,
            selected_kind: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "selected_kind")?,
                "selected_kind",
            )?,
            selected_id: ofdb_sql::decode(
                ofdb_sql::value(row, columns, "selected_id")?,
                "selected_id",
            )?,
            selected: ofdb_sql::decode(ofdb_sql::value(row, columns, "selected")?, "selected")?,
        })
    }
}
