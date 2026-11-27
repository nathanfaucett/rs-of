pub use query::{QueryError as Error, QueryErrorKind as ErrorKind};

#[cfg(any(
    feature = "redb",
    feature = "in-memory",
    all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"),
    feature = "sync"
))]
#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err supplies owned engine errors"
)]
pub fn from_engine(error: engine::EngineError) -> Error {
    use engine::EngineError;
    use query::QueryErrorKind;

    let kind = match &error {
        EngineError::TranslateError(_) => QueryErrorKind::Validation,
        EngineError::InvalidQuery(_) => QueryErrorKind::Rejected,
        EngineError::Unsupported(_) | EngineError::SyncDependencyUnavailable => {
            QueryErrorKind::Unsupported
        }
        EngineError::LimitExceeded => QueryErrorKind::LimitExceeded,
        EngineError::TransactionConflict | EngineError::Custom(_) => QueryErrorKind::Storage,
    };
    Error::new(kind, error.to_string())
}
