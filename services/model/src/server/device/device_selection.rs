use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::Deserialize;

use crate::id::Id;
use crate::storage::resource_catalog::ResourceKind;

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

impl fmt::Debug for SelectionRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectionRequest")
            .field("application_id", &self.application_id)
            .field("kind", &self.kind)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum SelectionKind {
    Database,
    Filesystem,
}

impl SelectionKind {
    #[must_use]
    pub const fn resource_kind(&self) -> ResourceKind {
        match self {
            Self::Database => ResourceKind::Database,
            Self::Filesystem => ResourceKind::FileSystem,
        }
    }

    #[must_use]
    pub const fn storage_audience(&self) -> &'static str {
        match self {
            Self::Database => "storage:databases",
            Self::Filesystem => "storage:filesystems",
        }
    }

    #[must_use]
    pub const fn policy_kind(&self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::Filesystem => "filesystem",
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::format;

    use super::{Id, SelectionKind, SelectionRequest};

    #[test]
    fn selection_request_debug_omits_the_storage_access_token() {
        let request = SelectionRequest {
            application_id: Id::from_u128(1),
            kind: SelectionKind::Database,
            id: Id::from_u128(2),
            storage_access_token: "selection-access-secret".into(),
        };
        let output = format!("{request:?}");

        assert!(output.contains("SelectionRequest"));
        assert!(output.contains("00000000-0000-0000-0000-000000000001"));
        assert!(output.contains("00000000-0000-0000-0000-000000000002"));
        assert!(output.contains("Database"));
        assert!(!output.contains("selection-access-secret"));
        assert!(!output.contains("storage_access_token"));
    }
}
