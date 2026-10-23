use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use core::error::Error;

use idp_model::contract::{ErrorCode, ErrorResponse};

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("deserialize error: {0}")]
    DeserializeError(#[from] serde::de::value::Error),

    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("{0}")]
    Other(#[source] Box<dyn Error + Send + Sync>),
}

impl DeviceError {
    pub fn other<E>(error: E) -> Self
    where
        E: Into<Box<dyn Error + Send + Sync>>,
    {
        Self::Other(error.into())
    }
}

impl From<DeviceError> for ErrorResponse {
    fn from(error: DeviceError) -> Self {
        Self::new(ErrorCode::ServerError).with_description(error.to_string())
    }
}

pub type DeviceResult<T> = Result<T, DeviceError>;
