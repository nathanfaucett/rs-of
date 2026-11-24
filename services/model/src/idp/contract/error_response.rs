#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

#[cfg(feature = "axum")]
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use key::error::KeyError;
use serde::{Deserialize, Serialize, de::value::Error as SerdeError};

use super::error_code::ErrorCode;

/// Standard OAuth 2.0 error response structure (RFC 6749 Section 5.2).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ErrorResponse {
    pub error: ErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_uri: Option<String>,
}

impl From<SerdeError> for ErrorResponse {
    fn from(error: SerdeError) -> Self {
        Self::new(ErrorCode::ServerError).with_description(error.to_string())
    }
}

impl From<KeyError> for ErrorResponse {
    fn from(error: KeyError) -> Self {
        Self::new(ErrorCode::ServerError).with_description(error.to_string())
    }
}

impl ErrorResponse {
    #[must_use]
    pub const fn new(code: ErrorCode) -> Self {
        Self {
            error: code,
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

#[cfg(feature = "axum")]
impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status = match self.error {
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::NotAuthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::InvalidRequest
            | ErrorCode::InvalidClient
            | ErrorCode::InvalidGrant
            | ErrorCode::UnsupportedGrantType
            | ErrorCode::InvalidScope
            | ErrorCode::InvalidResponseType
            | ErrorCode::UnsupportedResponseType => StatusCode::BAD_REQUEST,
            ErrorCode::UnauthorizedClient | ErrorCode::AccessDenied => StatusCode::FORBIDDEN,
            ErrorCode::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::TemporarilyUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::NotImplemented => StatusCode::NOT_IMPLEMENTED,
        };
        (status, Json(self)).into_response()
    }
}

pub type ErrorResponseResult<T> = Result<T, ErrorResponse>;
