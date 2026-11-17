use core::future::Future;

use crate::storage::contract::request::StorageRequest;
use crate::storage::contract::response::StorageResponse;

pub trait StorageSession: Send + Sync {
    fn execute_session(
        &self,
        request: StorageRequest,
    ) -> impl Future<Output = StorageResponse> + Send;
}
