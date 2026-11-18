use core::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DocumentType {
    Snapshot = 0,
    Incremental = 1,
    Metadata = 2,
}

impl fmt::Display for DocumentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Snapshot => write!(f, "Snapshot"),
            Self::Incremental => write!(f, "Incremental"),
            Self::Metadata => write!(f, "Metadata"),
        }
    }
}

impl DocumentType {
    #[must_use]
    pub const fn is_snapshot(self) -> bool {
        matches!(self, Self::Snapshot)
    }

    #[must_use]
    pub const fn is_incremental(self) -> bool {
        matches!(self, Self::Incremental)
    }

    #[must_use]
    pub const fn is_metadata(self) -> bool {
        matches!(self, Self::Metadata)
    }
}
