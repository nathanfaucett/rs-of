#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    string::{String, ToString},
};

use core::error::Error;

use serde::de::value::Error as SerdeError;

use crate::idp::contract::error_code::ErrorCode;
use crate::idp::contract::error_response::ErrorResponse;

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("deserialize error: {0}")]
    DeserializeError(SerdeError),

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

impl From<SerdeError> for DeviceError {
    fn from(error: SerdeError) -> Self {
        Self::DeserializeError(error)
    }
}
