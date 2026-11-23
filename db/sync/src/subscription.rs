use std::pin::Pin;

use btree::ChangeEvent;
use futures::Stream;

use crate::{
    protocol::{PushApplyResult, PushResult, PushSynchronizeConfig, PushSynchronizeError},
    transport::SyncTransport,
};

#[cfg(all(feature = "std", not(feature = "wasm")))]
use crate::sync::push_synchronize;
#[cfg(all(feature = "std", feature = "wasm"))]
use crate::wasm_sync::push_synchronize;

#[inline(never)]
fn sync_stage<F: Future>(make: impl FnOnce() -> F) -> Pin<Box<F>> {
    Box::pin(make())
}

/// Sync-owned continuous reconciliation over coalesced committed changes.
///
/// Construct before starting reconciliation. This subscription alone does not retain storage.
/// Cloning starts observing at the current committed state, like a new subscription.
#[derive(Clone, Debug)]
pub struct ChangeSubscription {
    changes: btree::ChangeSubscription,
}

impl From<btree::ChangeSubscription> for ChangeSubscription {
    fn from(changes: btree::ChangeSubscription) -> Self {
        Self { changes }
    }
}

impl ChangeSubscription {
    /// Returns a stream of committed changes with their incremental blobs.
    /// Each event is delivered exactly once per subscription in generation order.
    /// This is the push-based replacement for the old select/acknowledge/changed API.
    pub fn events(&mut self) -> impl Stream<Item = ChangeEvent> + '_ {
        self.changes.events()
    }

    /// Combine stores into one lifecycle. Changes to either schedule one shared pass.
    /// The subscription closes only after all combined backends close.
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        Self {
            changes: self.changes.merge(other.changes),
        }
    }

    /// Run push-based synchronization using the events stream.
    /// This replaces the deprecated `run()` method.
    ///
    /// # Arguments
    /// - `transport`: Bidirectional transport implementing `SyncTransport`
    /// - `apply_inbound`: Function to apply received `ChangeEvent` to local storage
    /// - `config`: Synchronization configuration
    ///
    /// # Returns
    /// `PushResult` with statistics, or `PushSynchronizeError` on failure.
    ///
    /// # Errors
    ///
    /// Returns an error when transport, protocol, or inbound apply processing fails.
    #[cfg(all(feature = "std", not(feature = "wasm")))]
    pub async fn push_synchronize<T, F, Fut>(
        mut self,
        transport: T,
        apply_inbound: F,
        config: PushSynchronizeConfig,
    ) -> Result<PushResult, PushSynchronizeError<T::Error>>
    where
        T: SyncTransport,
        F: FnMut(ChangeEvent) -> Fut + Send,
        Fut: Future<Output = Result<PushApplyResult, String>> + Send,
    {
        push_synchronize(transport, self.events(), apply_inbound, config).await
    }

    /// Runs push synchronization with browser-compatible futures.
    ///
    /// # Errors
    ///
    /// Returns an error when transport, protocol, or inbound apply processing fails.
    #[cfg(all(feature = "std", feature = "wasm"))]
    pub async fn push_synchronize<T, F, Fut>(
        mut self,
        transport: T,
        apply_inbound: F,
        config: PushSynchronizeConfig,
    ) -> Result<PushResult, PushSynchronizeError<T::Error>>
    where
        T: SyncTransport,
        F: FnMut(ChangeEvent) -> Fut,
        Fut: Future<Output = Result<PushApplyResult, String>>,
    {
        push_synchronize(transport, self.events(), apply_inbound, config).await
    }

    #[doc(hidden)]
    pub async fn closed(&self) {
        self.changes.closed().await;
    }
}
