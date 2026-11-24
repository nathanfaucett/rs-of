use core::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    /// Confidential clients are capable of maintaining the confidentiality of their credentials (e.g., server-side applications).
    Confidential = 0,
    /// Public clients are incapable of maintaining the confidentiality of their credentials (e.g., single-page apps, mobile apps).
    #[default]
    Public = 1,
}

impl fmt::Display for ClientType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Confidential => write!(f, "confidential"),
            Self::Public => write!(f, "public"),
        }
    }
}
