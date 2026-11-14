use alloc::string::String;

use serde::Deserialize;

use crate::idp::model::Id;
use crate::storage::ResourceKind;

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionRequest {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub application_id: Id,
    pub kind: SelectionKind,
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub id: Id,
    pub storage_access_token: String,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum SelectionKind {
    Database,
    Filesystem,
}

impl SelectionKind {
    pub fn resource_kind(&self) -> ResourceKind {
        match self {
            Self::Database => ResourceKind::Database,
            Self::Filesystem => ResourceKind::FileSystem,
        }
    }

    pub fn storage_audience(&self) -> &'static str {
        match self {
            Self::Database => "storage:databases",
            Self::Filesystem => "storage:filesystems",
        }
    }

    pub fn policy_kind(&self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::Filesystem => "filesystem",
        }
    }
}
