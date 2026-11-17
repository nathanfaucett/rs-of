use std::{
    ops::Bound,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use futures::Stream;
use ofdb_sql::{EngineResult, Kernel, KernelTransaction, RedbKernel};

#[derive(Default)]
pub(super) struct CatalogReads {
    tables: AtomicUsize,
    fields: AtomicUsize,
    signers: AtomicUsize,
    members: AtomicUsize,
}

impl CatalogReads {
    pub(super) fn reset(&self) {
        self.tables.store(0, Ordering::Relaxed);
        self.fields.store(0, Ordering::Relaxed);
        self.signers.store(0, Ordering::Relaxed);
        self.members.store(0, Ordering::Relaxed);
    }

    pub(super) fn trust_documents(&self) -> [usize; 2] {
        [
            self.signers.load(Ordering::Relaxed),
            self.members.load(Ordering::Relaxed),
        ]
    }

    pub(super) fn totals(&self) -> [usize; 2] {
        [
            self.tables.load(Ordering::Relaxed),
            self.fields.load(Ordering::Relaxed),
        ]
    }
}

#[derive(Clone)]
pub(super) struct CountingKernel {
    pub(super) inner: RedbKernel,
    pub(super) reads: Arc<CatalogReads>,
}

pub(super) struct CountingTransaction {
    inner: <RedbKernel as Kernel>::Transaction,
    pub(super) reads: Arc<CatalogReads>,
}

impl Kernel for CountingKernel {
    type Transaction = CountingTransaction;

    async fn transaction(&self) -> EngineResult<Self::Transaction> {
        Ok(CountingTransaction {
            inner: self.inner.transaction().await?,
            reads: Arc::clone(&self.reads),
        })
    }
}

impl KernelTransaction for CountingTransaction {
    async fn ensure_table(&mut self, table: &str) -> EngineResult<()> {
        self.inner.ensure_table(table).await
    }

    async fn drop_table(&mut self, table: &str) -> EngineResult<()> {
        self.inner.drop_table(table).await
    }

    async fn get_bytes(&self, table: &str, key: &[u8]) -> EngineResult<Option<Vec<u8>>> {
        self.inner.get_bytes(table, key).await
    }

    fn scan_bytes<'a>(
        &'a self,
        table: &'a str,
    ) -> impl Stream<Item = EngineResult<(Vec<u8>, Vec<u8>)>> + Send + 'a {
        self.scan_bytes_range(table, (Bound::Unbounded, Bound::Unbounded))
    }

    fn scan_bytes_range<'a>(
        &'a self,
        table: &'a str,
        range: (Bound<Vec<u8>>, Bound<Vec<u8>>),
    ) -> impl Stream<Item = EngineResult<(Vec<u8>, Vec<u8>)>> + Send + 'a {
        if matches!(range, (Bound::Unbounded, Bound::Unbounded)) {
            match table {
                "tables" => {
                    self.reads.tables.fetch_add(1, Ordering::Relaxed);
                }
                "table_fields" => {
                    self.reads.fields.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
        }
        if !matches!(range, (Bound::Unbounded, Bound::Unbounded)) {
            match table {
                "idp_replica_signers" => {
                    self.reads.signers.fetch_add(1, Ordering::Relaxed);
                }
                "idp_replica_members" => {
                    self.reads.members.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
        }
        self.inner.scan_bytes_range(table, range)
    }

    async fn put_bytes(&mut self, table: &str, key: Vec<u8>, value: Vec<u8>) -> EngineResult<()> {
        self.inner.put_bytes(table, key, value).await
    }

    async fn remove_bytes(&mut self, table: &str, key: &[u8]) -> EngineResult<Option<Vec<u8>>> {
        self.inner.remove_bytes(table, key).await
    }

    async fn commit(self) -> EngineResult<()> {
        self.inner.commit().await
    }

    async fn rollback(self) -> EngineResult<()> {
        self.inner.rollback().await
    }
}
