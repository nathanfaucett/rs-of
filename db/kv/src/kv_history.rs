use super::{Database, Storage};
use crate::history::{self, Config, Error, KvSnapshot, SyncRole, SyncTransport};
use btree::BTree;
use kv_store::KvStore;

impl Database {
    /// Exchanges native history with a checked transport.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, protocol, pre-apply, or history failure.
    pub async fn exchange_history<T, Q, U>(
        &self,
        transport: &mut T,
        role: SyncRole,
        config: Config,
        check: Q,
    ) -> Result<(), Error<T::Error>>
    where
        T: SyncTransport + platform::MaybeSend,
        T::Error: platform::MaybeSend,
        Q: FnMut() -> U + platform::MaybeSend,
        U: Future<Output = Result<(), String>> + platform::MaybeSend,
    {
        match &self.storage {
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => {
                history::synchronize_checked(store, transport, role, config, check).await
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(store) => {
                history::synchronize_checked(store, transport, role, config, check).await
            }
            #[cfg(feature = "redb")]
            Storage::Redb(resource) => resource
                .session(async {
                    let store = resource
                        .read()
                        .await
                        .map_err(|error| Error::PreApply(error.to_string()))?;
                    history::synchronize_checked(&*store, transport, role, config, check).await
                })
                .await
                .map_err(|error| Error::PreApply(error.to_string()))?,
        }
    }
    /// Exports the latest snapshot for every key.
    ///
    /// # Errors
    ///
    /// Returns an error for storage or document decoding failure.
    pub async fn export_snapshots(&self) -> Result<Vec<KvSnapshot>, crate::Error> {
        match &self.storage {
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => Box::pin(export(store)).await,
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(store) => export(store).await,
            #[cfg(feature = "redb")]
            Storage::Redb(resource) => {
                let guard = resource
                    .read()
                    .await
                    .map_err(|e| crate::Error::from(btree::BTreeError::custom(e)))?;
                let store = &*guard;
                let result = Box::pin(export(store)).await;
                drop(guard);
                result
            }
        }
    }
    /// Imports and reconciles snapshots.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid snapshots, missing history, or storage failure.
    pub async fn import_snapshots(&self, snapshots: Vec<KvSnapshot>) -> Result<(), crate::Error> {
        match &self.storage {
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => Box::pin(import(store, snapshots)).await,
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(store) => import(store, snapshots).await,
            #[cfg(feature = "redb")]
            Storage::Redb(resource) => {
                let guard = resource
                    .read()
                    .await
                    .map_err(|e| crate::Error::from(btree::BTreeError::custom(e)))?;
                let store = &*guard;
                let result = Box::pin(import(store, snapshots)).await;
                drop(guard);
                result
            }
        }
    }
}
async fn export<B: BTree<Vec<u8>, Vec<u8>>>(
    store: &KvStore<B>,
) -> Result<Vec<KvSnapshot>, crate::Error> {
    let tx = store.transaction().await?;
    let result = tx.export_snapshots().await;
    tx.rollback().await?;
    Ok(result?
        .into_iter()
        .map(|(key, payload)| KvSnapshot { key, payload })
        .collect())
}
async fn import<B: BTree<Vec<u8>, Vec<u8>>>(
    store: &KvStore<B>,
    snapshots: Vec<KvSnapshot>,
) -> Result<(), crate::Error> {
    let mut tx = store.transaction().await?;
    for snapshot in snapshots {
        if let Err(error) = tx.import_snapshot((snapshot.key, snapshot.payload)).await {
            tx.rollback().await?;
            return Err(error.into());
        }
    }
    tx.commit().await?;
    Ok(())
}
