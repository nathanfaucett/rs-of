use core::{fmt, str::from_utf8};

use query::QueryErrorKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryServiceError {
    Invalid(String),
    Rejected(String),
    Unsupported(&'static str),
    Storage(String),
    Internal(String),
}

#[must_use]
pub fn encode_error_detail(kind: QueryErrorKind, message: &str) -> Vec<u8> {
    let mut detail = Vec::with_capacity(message.len() + 1);
    detail.push(kind as u8);
    detail.extend_from_slice(message.as_bytes());
    detail
}

#[must_use]
pub fn decode_error_detail(detail: &[u8]) -> Option<(QueryErrorKind, String)> {
    let (&kind, message) = detail.split_first()?;
    let kind = QueryErrorKind::from_wire(kind)?;
    let message = from_utf8(message).ok()?.into();
    Some((kind, message))
}

impl fmt::Display for QueryServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(f, "invalid request: {message}"),
            Self::Rejected(message) => write!(f, "request rejected: {message}"),
            Self::Unsupported(message) => write!(f, "unsupported: {message}"),
            Self::Storage(message) => write!(f, "storage error: {message}"),
            Self::Internal(message) => write!(f, "internal error: {message}"),
        }
    }
}
