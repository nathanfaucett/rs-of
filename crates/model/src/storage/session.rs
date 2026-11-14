use core::future::Future;

use crate::storage::{StorageRequest, StorageResponse};

pub trait StorageSession: Send + Sync {
    fn execute_session(
        &self,
        request: StorageRequest,
    ) -> impl Future<Output = StorageResponse> + Send;
}
