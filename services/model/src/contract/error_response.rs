#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use serde::{Deserialize, Serialize, de::value::Error as SerdeError};

/// Standard OAuth 2.0 error response structure (RFC 6749 Section 5.2).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_uri: Option<String>,
}

impl From<SerdeError> for ErrorResponse {
    fn from(error: SerdeError) -> Self {
        Self::new("internal_server_error").with_description(error.to_string())
    }
}

impl ErrorResponse {
    #[must_use]
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            error_description: None,
            error_uri: None,
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.error_description = Some(description.into());
        self
    }

    #[must_use]
    pub fn with_error_uri(mut self, error_uri: impl Into<String>) -> Self {
        self.error_uri = Some(error_uri.into());
        self
    }
}
