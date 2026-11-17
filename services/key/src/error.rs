use core::error::Error;

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String};

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("bip32: {0}")]
    Bip32(bip32::Error),
    #[error("getrandom: {0}")]
    Random(getrandom::Error),
    #[error("bip39: {0}")]
    Bip39(bip39::Error),
    #[error("invalid derivation path: {0}")]
    InvalidDerivation(String),
    #[error("{0}")]
    Other(#[from] Box<dyn Error + Send + Sync>),
}

impl KeyError {
    pub fn invalid_derivation<T>(error: T) -> Self
    where
        T: Into<String>,
    {
        KeyError::InvalidDerivation(error.into())
    }

    pub fn other<E>(error: E) -> Self
    where
        E: Into<Box<dyn Error + Send + Sync>>,
    {
        KeyError::Other(error.into())
    }
}

impl From<bip32::Error> for KeyError {
    fn from(err: bip32::Error) -> Self {
        KeyError::Bip32(err)
    }
}

impl From<bip39::Error> for KeyError {
    fn from(err: bip39::Error) -> Self {
        KeyError::Bip39(err)
    }
}

impl From<getrandom::Error> for KeyError {
    fn from(err: getrandom::Error) -> Self {
        KeyError::Random(err)
    }
}

pub type KeyResult<T> = Result<T, KeyError>;
