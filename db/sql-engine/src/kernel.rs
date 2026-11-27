#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use core::ops::Bound;

use platform::{MaybeSend, MaybeSync};

use futures::Stream;

#[cfg(feature = "std")]
use btree::{ChangeEvent, ChangeSignal};

use crate::EngineResult;

pub trait KernelTransaction: MaybeSend + MaybeSync {
    /// Sets a cumulative budget before owned native read materialization. Unsupported backends fail closed.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend cannot enforce the requested limits.
    fn set_read_limits(&mut self, _entries: usize, _bytes: usize) -> EngineResult<()> {
        Err(crate::EngineError::Unsupported("native read limits"))
    }

    /// Checks a decoded row before scan accumulation. Backends without limits accept it.
    ///
    /// # Errors
    ///
    /// Returns an error when the row exceeds the configured read budget.
    fn check_read_row(&self, _row: &value::Row) -> EngineResult<()> {
        Ok(())
    }

    fn read_limit_exceeded(&self) -> bool {
        false
    }

    fn ensure_table(&mut self, table: &str) -> impl Future<Output = EngineResult<()>> + MaybeSend;
    fn drop_table(&mut self, table: &str) -> impl Future<Output = EngineResult<()>> + MaybeSend;

    /// Reads never create storage; a missing logical table reads as empty.
    fn get_bytes(
        &self,
        table: &str,
        key: &[u8],
    ) -> impl Future<Output = EngineResult<Option<Vec<u8>>>> + MaybeSend;
    /// Missing logical tables yield no entries without creating storage.
    fn scan_bytes<'a>(
        &'a self,
        table: &'a str,
    ) -> impl Stream<Item = EngineResult<(Vec<u8>, Vec<u8>)>> + MaybeSend + 'a;
    /// Missing logical tables yield no entries without creating storage.
    fn scan_bytes_range<'a>(
        &'a self,
        table: &'a str,
        range: (Bound<Vec<u8>>, Bound<Vec<u8>>),
    ) -> impl Stream<Item = EngineResult<(Vec<u8>, Vec<u8>)>> + MaybeSend + 'a;
    fn put_bytes(
        &mut self,
        table: &str,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> impl Future<Output = EngineResult<()>> + MaybeSend;
    fn remove_bytes(
        &mut self,
        table: &str,
        key: &[u8],
    ) -> impl Future<Output = EngineResult<Option<Vec<u8>>>> + MaybeSend;

    fn commit(self) -> impl Future<Output = EngineResult<()>> + MaybeSend;

    /// Runs a read-only check under actual writer admission before committing changes.
    ///
    /// # Errors
    ///
    /// Returns a check or storage error. Backends without a writer fence fail closed.
    fn commit_checked<Q, U, E>(self, _check: Q) -> impl Future<Output = Result<(), E>> + MaybeSend
    where
        Self: Sized,
        Q: FnOnce() -> U + MaybeSend,
        U: Future<Output = Result<(), E>> + MaybeSend,
        E: From<crate::EngineError> + MaybeSend,
    {
        core::future::ready(Err(crate::EngineError::Unsupported("checked commit").into()))
    }

    /// Publishes legacy events only after this transaction commits durably.
    ///
    /// # Errors
    ///
    /// Returns the transaction commit error and publishes no event on failure.
    #[cfg(feature = "std")]
    fn commit_with_events(
        self,
        events: Vec<ChangeEvent>,
        signal: ChangeSignal,
    ) -> impl Future<Output = EngineResult<()>> + MaybeSend
    where
        Self: Sized,
    {
        async move {
            self.commit().await?;
            signal.committed_with_events(events);
            Ok(())
        }
    }

    fn rollback(self) -> impl Future<Output = EngineResult<()>> + MaybeSend;
}

pub trait Kernel: MaybeSend + MaybeSync {
    type Transaction: KernelTransaction;

    /// Unsupported backends require caller-driven broad reconciliation.
    #[cfg(feature = "std")]
    fn subscribe_changes(&self) -> Option<btree::ChangeSubscription> {
        None
    }

    fn transaction(&self) -> impl Future<Output = EngineResult<Self::Transaction>> + MaybeSend;
}
