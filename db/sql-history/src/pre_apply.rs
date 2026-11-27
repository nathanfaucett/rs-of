use alloc::string::String;

use engine::{EngineError, KernelTransaction};
use thiserror::Error;

pub type CheckFuture<'a> = platform::BoxFuture<'a, Result<(), String>>;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub type Check<'a> = dyn FnMut() -> CheckFuture<'a> + Send + 'a;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub type Check<'a> = dyn FnMut() -> CheckFuture<'a> + 'a;

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("{0}")]
    Engine(#[from] EngineError),
    #[error("pre-apply check failed: {0}")]
    PreApply(String),
}
impl ApplyError {
    pub fn into_engine(self) -> EngineError {
        match self {
            Self::Engine(error) => error,
            Self::PreApply(reason) => EngineError::custom(reason),
        }
    }
}
pub async fn check_apply(check: &mut Check<'_>) -> Result<(), ApplyError> {
    check().await.map_err(ApplyError::PreApply)
}

impl<E> From<ApplyError> for crate::SyncError<E> {
    fn from(error: ApplyError) -> Self {
        match error {
            ApplyError::Engine(error) => Self::Engine(error),
            ApplyError::PreApply(reason) => Self::PreApply(reason),
        }
    }
}

// IndexedDB loads a snapshot after the pre-apply check. Recheck before issuing its write.
// Native checks stay outside writer transactions: they may need the same database.
pub async fn commit_apply<T: KernelTransaction>(
    transaction: T,
    _check: &mut Check<'_>,
) -> Result<(), ApplyError> {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    check_apply(_check).await?;
    transaction.commit().await?;
    Ok(())
}
