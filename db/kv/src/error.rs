use core::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    InvalidDocument,
    TypeMismatch,
    Conflict,
    CommitFailed,
    RollbackFailed,
    UnsupportedOperation,
    Storage,
    Internal,
}

#[cfg(feature = "remote")]
impl From<kv_proto::ErrorKind> for ErrorKind {
    fn from(kind: kv_proto::ErrorKind) -> Self {
        match kind {
            kv_proto::ErrorKind::InvalidDocument => Self::InvalidDocument,
            kv_proto::ErrorKind::TypeMismatch => Self::TypeMismatch,
            kv_proto::ErrorKind::Conflict => Self::Conflict,
            kv_proto::ErrorKind::CommitFailed => Self::CommitFailed,
            kv_proto::ErrorKind::RollbackFailed => Self::RollbackFailed,
            kv_proto::ErrorKind::UnsupportedOperation => Self::UnsupportedOperation,
            kv_proto::ErrorKind::Storage => Self::Storage,
            kv_proto::ErrorKind::Internal => Self::Internal,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    #[cfg(not(any(
        feature = "remote",
        feature = "in-memory",
        feature = "redb",
        feature = "indexeddb",
        feature = "sync",
        feature = "server"
    )))]
    Unconfigured,
    #[cfg(any(
        feature = "in-memory",
        feature = "redb",
        feature = "indexeddb",
        feature = "sync"
    ))]
    Storage { kind: ErrorKind, message: String },
    #[cfg(feature = "remote")]
    Connect(String),
    #[cfg(feature = "remote")]
    Transport(String),
    #[cfg(feature = "remote")]
    Query { kind: ErrorKind, message: String },
    #[cfg(any(
        feature = "remote",
        feature = "in-memory",
        feature = "redb",
        feature = "indexeddb"
    ))]
    Timeout,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(not(any(
                feature = "remote",
                feature = "in-memory",
                feature = "redb",
                feature = "indexeddb",
                feature = "sync",
                feature = "server"
            )))]
            Self::Unconfigured => f.write_str("KV facade has no enabled features"),
            #[cfg(any(
                feature = "in-memory",
                feature = "redb",
                feature = "indexeddb",
                feature = "sync"
            ))]
            Self::Storage { kind, message } => {
                write!(f, "KV storage error ({kind:?}): {message}")
            }
            #[cfg(feature = "remote")]
            Self::Connect(message) => write!(f, "KV connection error: {message}"),
            #[cfg(feature = "remote")]
            Self::Transport(message) => write!(f, "KV transport error: {message}"),
            #[cfg(feature = "remote")]
            Self::Query { kind, message } => {
                write!(f, "KV query error ({kind:?}): {message}")
            }
            #[cfg(any(
                feature = "remote",
                feature = "in-memory",
                feature = "redb",
                feature = "indexeddb"
            ))]
            Self::Timeout => f.write_str("KV request deadline exceeded"),
        }
    }
}

#[cfg(feature = "std")]
use std::{error::Error as StdError, io};

#[cfg(feature = "std")]
impl StdError for Error {}

#[cfg(any(
    feature = "in-memory",
    feature = "redb",
    feature = "indexeddb",
    feature = "sync"
))]
impl From<btree::BTreeError> for Error {
    fn from(error: btree::BTreeError) -> Self {
        use btree::BTreeError;

        let kind = match &error {
            BTreeError::InvalidDocument => ErrorKind::InvalidDocument,
            BTreeError::TypeMismatch => ErrorKind::TypeMismatch,
            BTreeError::Conflict => ErrorKind::Conflict,
            BTreeError::CommitFailed => ErrorKind::CommitFailed,
            BTreeError::RollbackFailed => ErrorKind::RollbackFailed,
            BTreeError::UnsupportedOperation => ErrorKind::UnsupportedOperation,
            BTreeError::Custom(_) => ErrorKind::Storage,
        };
        Self::Storage {
            kind,
            message: error.to_string(),
        }
    }
}

#[cfg(feature = "std")]
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Storage {
            kind: ErrorKind::Storage,
            message: error.to_string(),
        }
    }
}
