#[cfg(feature = "sync")]
use std::collections::BTreeMap;
#[cfg(any(
    feature = "redb",
    feature = "server",
    feature = "sync",
    feature = "wasm"
))]
use std::io;
#[cfg(feature = "redb")]
use std::path::Path;
#[cfg(all(feature = "server", unix))]
use std::path::PathBuf;
#[cfg(any(feature = "redb", feature = "sync"))]
use std::sync::Arc;
#[cfg(feature = "sync")]
use std::sync::Mutex;
#[cfg(feature = "server")]
use std::{error::Error as StdError, net::SocketAddr};

#[cfg(feature = "redb")]
use btree_redb::{
    Bytes, NativeResource, RedbByteBTree, SharedDatabase, open_unretired,
    remove_retired as remove_native_retired, table_definition,
};

use btree::BTree;
#[cfg(feature = "in-memory")]
use btree::InMemoryBTree;
use kv_store::KvStore;
use uuid::Uuid;

#[cfg(feature = "sync")]
use sync::{PushResult, PushSynchronizeConfig, PushSynchronizeError, SyncTransport};

use value::Value;

use crate::Error;
#[cfg(feature = "sync")]
use btree::ChangeEvent;

#[derive(Clone, Debug)]
pub enum Storage {
    #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
    IndexedDb(KvStore<btree_indexeddb::ByteBTree>),
    #[cfg(feature = "in-memory")]
    Memory(KvStore<InMemoryBTree<Vec<u8>, Vec<u8>>>),
    #[cfg(feature = "redb")]
    Redb(NativeResource<KvStore<RedbByteBTree>>),
}

#[derive(Debug)]
pub struct Database {
    pub(super) storage: Storage,
    #[cfg(feature = "sync")]
    changes: Option<btree::ChangeSignal>,
    #[cfg(feature = "sync")]
    storage_table: Option<String>,
}

impl Database {
    /// Shared hosts permanently fence and remove only the selected namespace, never the host file.
    /// Dedicated files permanently fence issued handles and opens, drain admitted operations,
    /// cancels sessions, rolls back idle transactions, then removes this dedicated file.
    /// Native hosting is not managed: an owner that attempted native hosting returns Unsupported.
    /// Unix-only; callers must exclusively own the pathname and marker namespace.
    /// The permanent `.ofdb-retired` marker is retained, including after success.
    #[cfg(feature = "redb")]
    /// Retires this resource and removes its owned persistence.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported storage or failed retirement and cleanup.
    #[cfg(feature = "redb")]
    pub async fn retire_and_remove(&self) -> io::Result<()> {
        let result = match &self.storage {
            Storage::Redb(resource) => resource.retire_and_remove().await,
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "not a native resource",
            )),
            #[cfg(feature = "in-memory")]
            Storage::Memory(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "not a native resource",
            )),
        };
        #[cfg(feature = "sync")]
        if result.is_ok() {
            if let Some(signal) = &self.changes {
                signal.close();
            }
        }
        result
    }

    /// Resumes cleanup of a permanently retired file, refusing live native owners.
    #[cfg(feature = "redb")]
    /// Removes a permanently retired database file when no live owner holds it.
    ///
    /// # Errors
    ///
    /// Returns an error for a live owner, identity mismatch, or cleanup failure.
    #[cfg(feature = "redb")]
    pub fn remove_retired(path: impl AsRef<Path>) -> io::Result<()> {
        remove_native_retired(path)
    }

    /// Creates an owned query-only handle that shares this database's storage.
    /// The client remains usable after this handle is dropped.
    #[must_use]
    pub fn client(&self) -> crate::Client {
        crate::Client::embedded(self.clone_shared())
    }

    /// Creates the sync-owned lifecycle for committed changes, including remote apply.
    /// `ChangeSubscription::events` returns a stream of committed changes.
    /// Merge with a SQL subscription to reconcile both stores in one callback.
    /// Dropping the stream cancels its in-flight work. Returns Unsupported for memory.
    /// `IndexedDB` subscriptions observe this opened backend and its shared clients,
    /// not other tabs/opens. External-write gaps require broad recovery.
    #[cfg(feature = "sync")]
    /// Subscribes to committed changes for synchronization.
    ///
    /// # Errors
    ///
    /// Returns an error when the backend cannot retain committed changes.
    #[cfg(feature = "sync")]
    pub fn sync_changes(&self) -> io::Result<sync::ChangeSubscription> {
        self.changes
            .clone()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    "committed changes require a persistent database",
                )
            })
            .map(|signal| signal.subscribe().into())
    }

    pub(crate) fn clone_shared(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            #[cfg(feature = "sync")]
            changes: self.changes.clone(),
            #[cfg(feature = "sync")]
            storage_table: self.storage_table.clone(),
        }
    }

    /// Opens durable browser KV storage; never falls back to memory.
    /// Each operation materializes a whole-database snapshot. Mutating stale
    /// transactions fail with Conflict and are not replayed. Issued commits
    /// remain observed after cancellation; timeout does not prove rollback.
    #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
    /// Opens durable IndexedDB storage.
    ///
    /// # Errors
    ///
    /// Returns an error when browser storage cannot be opened or initialized.
    #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
    pub async fn open_indexeddb(name: &str) -> Result<Self, Error> {
        let database = btree_indexeddb::Database::open(name, "kv").await?;
        let tree = btree_indexeddb::ByteBTree::new(database, "ofdb-kv");
        let store = KvStore::new(tree, uuid_timestamp);
        #[cfg(feature = "sync")]
        let changes = Some(store.change_signal().clone());
        Ok(Self {
            storage: Storage::IndexedDb(store),
            #[cfg(feature = "sync")]
            changes,
            #[cfg(feature = "sync")]
            storage_table: Some("ofdb-kv".into()),
        })
    }

    #[cfg(feature = "in-memory")]
    pub fn in_memory() -> Self {
        let store = KvStore::new(InMemoryBTree::new(), uuid_timestamp);
        #[cfg(feature = "sync")]
        let changes = Some(store.change_signal().clone());
        Self {
            storage: Storage::Memory(store),
            #[cfg(feature = "sync")]
            changes,
            #[cfg(feature = "sync")]
            storage_table: None,
        }
    }

    /// Opens an isolated KV store in a shared host. Retirement never removes the host file.
    #[cfg(feature = "redb")]
    /// Opens an isolated KV namespace in a shared host.
    ///
    /// # Errors
    ///
    /// Returns an error for namespace validation or resource initialization failure.
    #[cfg(feature = "redb")]
    pub fn open_shared(host: &SharedDatabase, name: &str) -> Result<Self, Error> {
        let namespace = host
            .namespace(&format!("kv:{name}"))
            .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
        #[cfg(feature = "sync")]
        let table = namespace.table("kv");
        let store = KvStore::new(namespace.tree("kv"), uuid_timestamp);
        let resource = NativeResource::new_shared(namespace, store)
            .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
        #[cfg(feature = "sync")]
        let changes = Some(resource.inspect(|store| store.change_signal().clone())?);
        Ok(Self {
            storage: Storage::Redb(resource),
            #[cfg(feature = "sync")]
            changes,
            #[cfg(feature = "sync")]
            storage_table: Some(table),
        })
    }

    /// Opens a durable KV database.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid storage, schema incompatibility, or resource initialization.
    #[cfg(feature = "redb")]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::open_with_table(path, "ofdb-kv")
    }

    #[cfg(feature = "redb")]
    #[allow(
        clippy::map_err_ignore,
        reason = "an incompatible existing table has one stable schema error"
    )]
    /// Opens a durable KV database with a specific table name.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid storage, schema incompatibility, or resource initialization.
    #[cfg(feature = "redb")]
    pub fn open_with_table(
        path: impl AsRef<Path>,
        table: impl Into<String>,
    ) -> Result<Self, Error> {
        let table = table.into();
        let path = path.as_ref();
        let existed = path.exists();
        let db = open_unretired(path, !existed)
            .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
        let definition = table_definition::<Bytes, Vec<u8>>(&table);
        if existed {
            use redb::ReadableDatabase;

            let transaction = db
                .begin_read()
                .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
            transaction
                .open_table(definition)
                .map_err(|_| Error::Storage {
                    kind: crate::ErrorKind::Storage,
                    message: "unsupported metadata store schema".into(),
                })?;
        }
        let transaction = db
            .begin_write()
            .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
        transaction
            .open_table(definition)
            .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
        transaction
            .commit()
            .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
        let tree = RedbByteBTree::new(Arc::new(db), table.clone());
        let store = KvStore::new(tree, uuid_timestamp);
        #[cfg(feature = "sync")]
        let changes = Some(store.change_signal().clone());
        Ok(Self {
            storage: Storage::Redb(
                NativeResource::new(path, store)
                    .map_err(|error| Error::from(btree::BTreeError::custom(error)))?,
            ),
            #[cfg(feature = "sync")]
            changes,
            #[cfg(feature = "sync")]
            storage_table: Some(table),
        })
    }

    pub(crate) async fn query_get(&self, key: &str) -> Result<Option<Value>, Error> {
        let now = current_time_millis();
        match &self.storage {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(store) => read_get(store, key, now).await,
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => Box::pin(read_get(store, key, now)).await,
            #[cfg(feature = "redb")]
            Storage::Redb(store) => {
                let store = store
                    .read()
                    .await
                    .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
                Box::pin(read_get(&*store, key, now)).await
            }
        }
    }

    pub(crate) async fn query_set(
        &self,
        key: &str,
        value: Value,
        expires_at: Option<i64>,
    ) -> Result<(), Error> {
        match &self.storage {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(store) => write_set(store, key, value, expires_at).await,
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => Box::pin(write_set(store, key, value, expires_at)).await,
            #[cfg(feature = "redb")]
            Storage::Redb(store) => {
                let store = store
                    .read()
                    .await
                    .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
                Box::pin(write_set(&*store, key, value, expires_at)).await
            }
        }
    }

    pub(crate) async fn query_delete(&self, key: &str) -> Result<(), Error> {
        match &self.storage {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(store) => write_delete(store, key).await,
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => Box::pin(write_delete(store, key)).await,
            #[cfg(feature = "redb")]
            Storage::Redb(store) => {
                let store = store
                    .read()
                    .await
                    .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
                Box::pin(write_delete(&*store, key)).await
            }
        }
    }

    pub(crate) async fn query_scan(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Vec<(String, Value)>, Error> {
        let now = current_time_millis();
        match &self.storage {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(store) => read_scan(store, start, end, now).await,
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => Box::pin(read_scan(store, start, end, now)).await,
            #[cfg(feature = "redb")]
            Storage::Redb(store) => {
                let store = store
                    .read()
                    .await
                    .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
                Box::pin(read_scan(&*store, start, end, now)).await
            }
        }
    }

    pub(crate) async fn query_scan_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, Value)>, Error> {
        let now = current_time_millis();
        match &self.storage {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(store) => read_prefix(store, prefix, now).await,
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => Box::pin(read_prefix(store, prefix, now)).await,
            #[cfg(feature = "redb")]
            Storage::Redb(store) => {
                let store = store
                    .read()
                    .await
                    .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
                Box::pin(read_prefix(&*store, prefix, now)).await
            }
        }
    }

    pub(crate) async fn query_scan_all(&self) -> Result<Vec<(String, Value)>, Error> {
        let now = current_time_millis();
        match &self.storage {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(store) => read_all(store, now).await,
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => Box::pin(read_all(store, now)).await,
            #[cfg(feature = "redb")]
            Storage::Redb(store) => {
                let store = store
                    .read()
                    .await
                    .map_err(|error| Error::from(btree::BTreeError::custom(error)))?;
                Box::pin(read_all(&*store, now)).await
            }
        }
    }

    #[cfg(feature = "server")]
    /// Serves KV requests over TCP until shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error for listener setup, transport, or hosting failure.
    #[cfg(feature = "server")]
    pub async fn serve_tcp(
        &self,
        address: SocketAddr,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.serve_tcp_with_tls(address, None, shutdown).await
    }

    #[cfg(feature = "server")]
    /// Serves KV requests over TCP with optional TLS.
    ///
    /// # Errors
    ///
    /// Returns an error for TLS configuration, listener setup, transport, or hosting failure.
    #[cfg(feature = "server")]
    pub async fn serve_tcp_with_tls(
        &self,
        address: SocketAddr,
        identity: Option<(Vec<u8>, Vec<u8>)>,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.serve_tcp_with_client_ca(address, identity, None, shutdown)
            .await
    }

    #[cfg(feature = "server")]
    /// Serves KV requests over TCP with optional TLS and client CA validation.
    ///
    /// # Errors
    ///
    /// Returns an error for TLS configuration, listener setup, transport, or hosting failure.
    #[cfg(feature = "server")]
    pub async fn serve_tcp_with_client_ca(
        &self,
        address: SocketAddr,
        identity: Option<(Vec<u8>, Vec<u8>)>,
        client_ca: Option<Vec<u8>>,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        match &self.storage {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Storage::IndexedDb(_) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "native hosting is unavailable for IndexedDB",
            )
            .into()),
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => {
                let server = kv_server::Server::tcp(address, store.clone());
                let server = match identity {
                    Some((certificate, private_key)) => {
                        server.tls_identity(certificate, private_key)
                    }
                    None => server,
                };
                let server = match client_ca.clone() {
                    Some(certificate) => server.tls_client_ca(certificate),
                    None => server,
                };
                server.serve(shutdown).await
            }
            #[cfg(feature = "redb")]
            Storage::Redb(store) => {
                store
                    .hosting(async {
                        let store = store.hosting_backend().await?;
                        let server = kv_server::Server::tcp(address, store.clone());
                        let server = match identity {
                            Some((certificate, private_key)) => {
                                server.tls_identity(certificate, private_key)
                            }
                            None => server,
                        };
                        let server = match client_ca {
                            Some(certificate) => server.tls_client_ca(certificate),
                            None => server,
                        };
                        drop(store);
                        server.serve(shutdown).await
                    })
                    .await?
            }
        }
    }

    #[cfg(all(feature = "server", unix))]
    /// Serves KV requests over a Unix socket until shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error for listener setup, transport, or hosting failure.
    #[cfg(all(feature = "server", unix))]
    pub async fn serve_unix(
        &self,
        path: impl Into<PathBuf> + Send,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        match &self.storage {
            #[cfg(feature = "in-memory")]
            Storage::Memory(store) => {
                kv_server::Server::unix(path, store.clone())
                    .serve(shutdown)
                    .await
            }
            #[cfg(feature = "redb")]
            Storage::Redb(store) => {
                store
                    .hosting(async {
                        let store = store.hosting_backend().await?;
                        kv_server::Server::unix(path, store.clone())
                            .serve(shutdown)
                            .await
                    })
                    .await?
            }
        }
    }

    /// Synchronization: streams local changes to peer and applies remote changes.
    /// Uses the store's change subscription for outbound events and imports inbound events directly.
    #[cfg(feature = "sync")]
    /// Streams committed changes and applies inbound changes.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, protocol, or inbound apply failure.
    #[cfg(feature = "sync")]
    pub async fn synchronize<T: SyncTransport>(
        &self,
        transport: T,
        config: PushSynchronizeConfig,
    ) -> Result<PushResult, PushSynchronizeError<T::Error>> {
        // Get the change subscription for outbound events
        let subscription = self
            .sync_changes()
            .map_err(|e| PushSynchronizeError::Apply(e.to_string()))?;

        // Clone storage for the closure
        let storage = self.storage.clone();
        let pending = Arc::new(Mutex::new(BTreeMap::new()));
        let inbound_pending = Arc::clone(&pending);

        // Apply inbound events by importing changes into the store
        let apply_inbound = move |event: ChangeEvent| {
            let storage = storage.clone();
            let pending = Arc::clone(&inbound_pending);
            async move {
                let key = event.key.clone();
                let change =
                    automerge::Change::from_bytes(event.blob).map_err(|error| error.to_string())?;
                let change = kv_store::NativeChange {
                    hash: change.hash().0,
                    deps: change.deps().iter().map(|hash| hash.0).collect(),
                    payload: change.raw_bytes().to_vec(),
                };
                #[allow(
                    unused_variables,
                    reason = "only IndexedDB currently applies inbound changes"
                )]
                let changes = {
                    let mut pending = pending.lock().map_err(|error| error.to_string())?;
                    let changes = pending.entry(key.clone()).or_insert_with(Vec::new);
                    changes.push(change);
                    let changes = changes.clone();
                    drop(pending);
                    changes
                };
                match &storage {
                    #[cfg(all(
                        feature = "indexeddb",
                        target_arch = "wasm32",
                        target_os = "unknown"
                    ))]
                    Storage::IndexedDb(store) => {
                        let doc_id = key.key.clone(); // StorageKey.key = document ID for KV
                        let mut tx = store.transaction().await.map_err(|e| e.to_string())?;
                        let result = tx
                            .import_changes(&doc_id, &changes)
                            .await
                            .map_err(|error| error.to_string())?;
                        if result == kv_store::ChangeApply::MissingDependencies {
                            return Ok(sync::PushApplyResult::Deferred);
                        }
                        tx.commit().await.map_err(|e| e.to_string())?;
                        pending
                            .lock()
                            .map_err(|error| error.to_string())?
                            .remove(&key);
                        Ok(sync::PushApplyResult::Applied)
                    }
                    #[cfg(feature = "in-memory")]
                    Storage::Memory(_store) => Err(format!(
                        "In-memory storage not yet supported for synchronize (key {key:?})"
                    )),
                    #[cfg(feature = "redb")]
                    Storage::Redb(_store) => Err(format!(
                        "Redb storage not yet supported for synchronize (key {key:?})"
                    )),
                }
            }
        };

        let result = subscription
            .push_synchronize(transport, apply_inbound, config)
            .await?;
        let pending = pending
            .lock()
            .map_err(|error| PushSynchronizeError::Apply(error.to_string()))?;
        if pending.is_empty() {
            Ok(result)
        } else {
            Err(PushSynchronizeError::Apply(
                "inbound changes have unresolved dependencies".into(),
            ))
        }
    }
}

async fn read_get<B: BTree<Vec<u8>, Vec<u8>>>(
    store: &KvStore<B>,
    key: &str,
    now: i64,
) -> Result<Option<Value>, Error> {
    let transaction = store.transaction().await?;
    let result = transaction.get(key, now).await?;
    transaction.rollback().await?;
    Ok(result)
}

async fn write_set<B: BTree<Vec<u8>, Vec<u8>>>(
    store: &KvStore<B>,
    key: &str,
    value: Value,
    expires_at: Option<i64>,
) -> Result<(), Error> {
    let mut transaction = store.transaction().await?;
    transaction.set(key, value, expires_at).await?;
    transaction.commit().await?;
    Ok(())
}

async fn write_delete<B: BTree<Vec<u8>, Vec<u8>>>(
    store: &KvStore<B>,
    key: &str,
) -> Result<(), Error> {
    let mut transaction = store.transaction().await?;
    transaction.delete(key).await?;
    transaction.commit().await?;
    Ok(())
}

async fn read_scan<B: BTree<Vec<u8>, Vec<u8>>>(
    store: &KvStore<B>,
    start: &str,
    end: &str,
    now: i64,
) -> Result<Vec<(String, Value)>, Error> {
    let transaction = store.transaction().await?;
    let result = transaction
        .scan(start.to_owned()..end.to_owned(), now)
        .await?;
    transaction.rollback().await?;
    Ok(result)
}

async fn read_prefix<B: BTree<Vec<u8>, Vec<u8>>>(
    store: &KvStore<B>,
    prefix: &str,
    now: i64,
) -> Result<Vec<(String, Value)>, Error> {
    let transaction = store.transaction().await?;
    let result = transaction.scan_prefix(prefix, now).await?;
    transaction.rollback().await?;
    Ok(result)
}

async fn read_all<B: BTree<Vec<u8>, Vec<u8>>>(
    store: &KvStore<B>,
    now: i64,
) -> Result<Vec<(String, Value)>, Error> {
    let transaction = store.transaction().await?;
    let result = transaction.scan_all(now).await?;
    transaction.rollback().await?;
    Ok(result)
}

fn uuid_timestamp() -> uuid::Timestamp {
    Uuid::now_v7()
        .get_timestamp()
        .expect("UUIDv7 has a timestamp")
}

fn current_time_millis() -> i64 {
    platform::unix_millis()
}

#[cfg(all(test, feature = "in-memory"))]
mod tests {
    use futures::executor::block_on;
    use value::Value;

    use super::Database;

    #[test]
    fn facade_preserves_expiry_and_scan_contract() {
        block_on(async {
            let database = Database::in_memory();
            let client = database.client();
            Box::pin(client.set("a", Value::Blob(vec![1]), Some(0)))
                .await
                .expect("set expired value");
            assert_eq!(
                Box::pin(client.get("a")).await.expect("read expired value"),
                None
            );
            Box::pin(client.set("a", Value::Blob(vec![1]), None))
                .await
                .expect("clear expiry");
            Box::pin(client.set("ab", Value::Blob(vec![2]), None))
                .await
                .expect("set second value");
            assert_eq!(
                Box::pin(client.scan("a", "ab"))
                    .await
                    .expect("exclusive range"),
                vec![("a".into(), Value::Blob(vec![1]))]
            );
            assert_eq!(
                Box::pin(client.scan_prefix(""))
                    .await
                    .expect("empty prefix matches all")
                    .len(),
                2
            );
            Box::pin(client.delete("missing"))
                .await
                .expect("delete missing key");
        });
    }
}

#[cfg(feature = "sync")]
#[path = "kv_history.rs"]
mod kv_history;
