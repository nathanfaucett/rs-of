use std::{error::Error as StdError, fmt, io};

#[derive(Debug)]
pub enum Error {
    InvalidPath,
    InvalidResourceId,
    NotFound,
    NotDirectory,
    IsDirectory,
    TypeConflict,
    AlreadyExists,
    InvalidChunkSize,
    InvalidReadLength,
    InvalidScanLimit,
    InvalidScanCursor,
    StaleRevision,
    ContentUnavailable,
    PassthroughWrite,
    Offline,
    PermissionDenied,
    Cancelled,
    NoPeers,
    NoReachableProvider,
    UnsupportedSyncProtocolVersion(u8),
    SyncFrameTooLarge,
    InvalidSyncFrame(String),
    Io(io::Error),
    Metadata(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath => {
                f.write_str("path must be relative and cannot use reserved components")
            }
            Self::InvalidResourceId => f.write_str("filesystem ID must be a UUIDv7"),
            Self::NotFound => f.write_str("path was not found"),
            Self::NotDirectory => f.write_str("path is not a directory"),
            Self::IsDirectory => f.write_str("path names a directory"),
            Self::TypeConflict => f.write_str("path conflicts with an existing entry type"),
            Self::AlreadyExists => f.write_str("destination already exists"),
            Self::InvalidChunkSize => f.write_str("chunk size must not be zero"),
            Self::InvalidReadLength => f.write_str("read length is too large"),
            Self::InvalidScanLimit => f.write_str("scan limit is too large"),
            Self::InvalidScanCursor => f.write_str("scan cursor is invalid"),
            Self::StaleRevision => f.write_str("file handle revision is stale"),
            Self::ContentUnavailable => f.write_str("content is unavailable"),
            Self::PassthroughWrite => f.write_str("writes require full residency"),
            Self::Offline => f.write_str("storage is offline"),
            Self::PermissionDenied => f.write_str("operation denied by local policy"),
            Self::Cancelled => f.write_str("sync execution cancelled"),
            Self::NoPeers => f.write_str("no peers are online"),
            Self::NoReachableProvider => {
                f.write_str("no reachable provider advertises the content")
            }
            Self::UnsupportedSyncProtocolVersion(version) => {
                write!(f, "unsupported filesystem sync protocol version: {version}")
            }
            Self::SyncFrameTooLarge => f.write_str("filesystem sync frame exceeds size limit"),
            Self::InvalidSyncFrame(error) => {
                write!(f, "invalid filesystem sync frame: {error}")
            }
            Self::Io(error) => error.fmt(f),
            Self::Metadata(error) => write!(f, "metadata error: {error}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
