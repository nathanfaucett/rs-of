use std::sync::Arc;

use async_lock::Mutex;
use redb::Database;

use btree::{BTreeError, BTreeResult, ChangeSignal, ChangeSubscription};

use crate::{
    RedbBTreeScopedTransaction,
    redb::{RedbKey, RedbValue},
    transaction::NativeWriteTransaction,
};

#[derive(Clone, Debug)]
pub struct RedbDatabase {
    inner: Arc<Database>,
    admission: Arc<Mutex<()>>,
    changes: ChangeSignal,
    shared: bool,
}

impl RedbDatabase {
    #[must_use]
    pub fn new(inner: Arc<Database>) -> Self {
        Self {
            inner,
            admission: Arc::new(Mutex::new(())),
            changes: ChangeSignal::default(),
            shared: false,
        }
    }

    pub fn with_admission(inner: Arc<Database>, admission: Arc<Mutex<()>>) -> Self {
        Self {
            inner,
            admission,
            changes: ChangeSignal::default(),
            shared: true,
        }
    }

    /// Subscribes to successfully committed storage changes.
    #[must_use]
    pub fn subscribe_changes(&self) -> ChangeSubscription {
        self.changes.subscribe()
    }

    /// Starts a native write transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when Redb cannot begin or share the transaction.
    pub async fn transaction(&self) -> BTreeResult<RedbDatabaseTransaction> {
        if self.shared {
            return Ok(RedbDatabaseTransaction {
                inner: NativeWriteTransaction::new_shared(
                    Arc::clone(&self.inner),
                    Arc::clone(&self.admission),
                    self.changes.clone(),
                )?,
            });
        }
        let admission = self.admission.lock_arc().await;
        Ok(RedbDatabaseTransaction {
            inner: NativeWriteTransaction::new(
                self.inner.begin_write().map_err(BTreeError::custom)?,
                admission,
                self.changes.clone(),
            ),
        })
    }
}

#[derive(Debug)]
pub struct RedbDatabaseTransaction {
    inner: NativeWriteTransaction,
}

impl RedbDatabaseTransaction {
    /// Sets one cumulative native read budget; repeated configuration is rejected.
    ///
    /// # Errors
    ///
    /// Returns an error when a read budget has already been configured.
    pub const fn set_read_limits(&mut self, entries: usize, bytes: usize) -> BTreeResult<()> {
        self.inner.read_budget.set(entries, bytes)
    }

    pub fn read_limit_exceeded(&self) -> bool {
        self.inner.read_budget.exhausted()
    }

    pub fn table<K, V>(&self, name: impl Into<String>) -> RedbBTreeScopedTransaction<'_, K, V> {
        RedbBTreeScopedTransaction::new(&self.inner, name)
    }

    /// Creates or validates a table in this transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when the table cannot be created or its types conflict.
    pub fn create_table<K: RedbKey, V: RedbValue>(&self, name: &str) -> BTreeResult<()> {
        self.inner.ensure_table::<K, V>(name)?;
        Ok(())
    }

    /// Drops a table in this transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when the table cannot be dropped or its types conflict.
    pub fn drop_table<K: RedbKey, V: RedbValue>(&self, name: &str) -> BTreeResult<bool> {
        self.inner.drop_table::<K, V>(name)
    }

    /// Commits the transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when persistence or conflict validation fails.
    pub async fn commit(self) -> BTreeResult<()> {
        self.inner.commit().await
    }

    /// Runs an asynchronous read-only check under the actual writer before persisting changes.
    /// Transactions with no shared writes do not acquire the writer or run the check.
    ///
    /// # Errors
    ///
    /// Returns a storage error on admission, commit, or rollback failure. A denied check is
    /// returned separately, without persisting changes.
    pub async fn commit_checked<Q, U, E>(self, check: Q) -> BTreeResult<Result<(), E>>
    where
        Q: FnOnce() -> U,
        U: Future<Output = Result<(), E>>,
    {
        self.inner.commit_checked(check).await
    }
    /// Rolls back the transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when rollback fails.
    pub fn rollback(self) -> BTreeResult<()> {
        self.inner.rollback()
    }
}
