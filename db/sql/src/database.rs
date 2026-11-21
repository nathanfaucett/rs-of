#[cfg(any(
    feature = "redb",
    feature = "in-memory",
    all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"),
    feature = "sync"
))]
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use core::{fmt, pin::Pin};

#[cfg(any(feature = "redb", feature = "server", feature = "sync"))]
use std::io;
#[cfg(all(feature = "server", unix))]
use std::path::PathBuf;
#[cfg(feature = "server")]
use std::{error::Error as StdError, net::SocketAddr};
#[cfg(feature = "redb")]
use std::{path::Path, sync::Arc};

use crate::error::from_engine;
use engine::Engine;
#[cfg(any(
    feature = "redb",
    feature = "in-memory",
    all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"),
))]
use engine::EngineTransaction;
#[cfg(feature = "in-memory")]
use engine::InMemoryKernel;
use query::QueryError;
use query::{QueryParams, QueryResult, Statement, Translator};

#[cfg(feature = "sync")]
use sync::{ChangeEvent, PushResult, PushSynchronizeConfig, PushSynchronizeError, SyncTransport};

use value::{FromRow, Row, Value};

// Keep concrete engine conflict futures out of the caller's poll frame during construction.
#[cfg(any(
    feature = "redb",
    feature = "in-memory",
    all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown")
))]
#[inline(never)]
fn conflict_stage<F: Future>(make: impl FnOnce() -> F) -> Pin<Box<F>> {
    Box::pin(make())
}

#[cfg(any(
    feature = "redb",
    feature = "in-memory",
    all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"),
    feature = "sync"
))]
use engine_automerge::AutomergeRowCodec;

#[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
use engine_indexeddb::IndexedDbKernel;
#[cfg(feature = "redb")]
use engine_redb::RedbKernel;

/// An application-facing database using the Automerge row codec.
///
/// For a single UUID primary key, omit the key or use `DEFAULT` on insert to
/// generate an RFC `UUIDv7`. Use `RETURNING` to read it. Explicit keys are rejected
/// for both embedded and remote Clients. Natural and composite keys are unchanged.
/// Keys are immutable and tombstoned keys cannot revive.
///
/// Replication retains peer `UUIDv7` identities. Use fresh stores with this contract;
/// legacy `UUIDv5` Logical Rows and their histories are unsupported.
#[derive(Clone, Debug)]
pub struct Database {
    backend: DatabaseBackend,
    #[cfg(feature = "sync")]
    changes: Option<btree::ChangeSignal>,
}

#[derive(Clone)]
enum DatabaseBackend {
    #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
    IndexedDb(Engine<IndexedDbKernel, AutomergeRowCodec>),
    #[cfg(feature = "redb")]
    File(engine_redb::NativeResource<Engine<RedbKernel, AutomergeRowCodec>>),
    #[cfg(feature = "in-memory")]
    InMemory(Engine<InMemoryKernel, AutomergeRowCodec>),
}

impl fmt::Debug for DatabaseBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Self::IndexedDb(_) => f.debug_tuple("IndexedDb").field(&"...").finish(),
            #[cfg(feature = "redb")]
            Self::File(_) => f.debug_tuple("File").field(&"...").finish(),
            #[cfg(feature = "in-memory")]
            Self::InMemory(_) => f.debug_tuple("InMemory").field(&"...").finish(),
        }
    }
}

pub enum EmbeddedTransaction {
    #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
    IndexedDb(EngineTransaction<IndexedDbKernel, AutomergeRowCodec>),
    #[cfg(feature = "redb")]
    File(
        engine_redb::NativeTransaction<
            Engine<RedbKernel, AutomergeRowCodec>,
            Box<EngineTransaction<RedbKernel, AutomergeRowCodec>>,
        >,
    ),
    #[cfg(feature = "in-memory")]
    InMemory(EngineTransaction<InMemoryKernel, AutomergeRowCodec>),
}

impl EmbeddedTransaction {
    pub(crate) async fn set_read_limits(&mut self, entries: usize, bytes: usize) -> SqlResult<()> {
        match self {
            #[cfg(feature = "redb")]
            Self::File(transaction) => transaction
                .read()
                .await
                .map_err(native_error)?
                .set_read_limits(entries, bytes)
                .map_err(from_engine),
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Self::IndexedDb(transaction) => transaction
                .set_read_limits(entries, bytes)
                .map_err(from_engine),
            #[cfg(feature = "in-memory")]
            Self::InMemory(transaction) => transaction
                .set_read_limits(entries, bytes)
                .map_err(from_engine),
        }
    }

    pub(crate) async fn row_conflicts(&mut self, table: &str, key: &Row) -> SqlResult<Vec<String>> {
        match self {
            #[cfg(feature = "redb")]
            Self::File(transaction) => {
                let mut transaction = transaction.read().await.map_err(native_error)?;
                conflict_stage(|| transaction.row_conflicts(table, key))
                    .await
                    .map_err(from_engine)
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Self::IndexedDb(transaction) => {
                conflict_stage(|| transaction.row_conflicts(table, key))
                    .await
                    .map_err(from_engine)
            }
            #[cfg(feature = "in-memory")]
            Self::InMemory(transaction) => conflict_stage(|| transaction.row_conflicts(table, key))
                .await
                .map_err(from_engine),
        }
    }

    pub(crate) async fn row_conflict_values(
        &self,
        table: &str,
        key: &Row,
    ) -> SqlResult<Vec<(String, Vec<Value>)>> {
        match self {
            #[cfg(feature = "redb")]
            Self::File(transaction) => {
                let transaction = transaction.read().await.map_err(native_error)?;
                conflict_stage(|| transaction.row_conflict_values(table, key))
                    .await
                    .map_err(from_engine)
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Self::IndexedDb(transaction) => {
                conflict_stage(|| transaction.row_conflict_values(table, key))
                    .await
                    .map_err(from_engine)
            }
            #[cfg(feature = "in-memory")]
            Self::InMemory(transaction) => {
                conflict_stage(|| transaction.row_conflict_values(table, key))
                    .await
                    .map_err(from_engine)
            }
        }
    }

    pub(crate) async fn execute(
        &mut self,
        statements: Vec<Statement>,
    ) -> SqlResult<Vec<QueryResult>> {
        match self {
            #[cfg(feature = "redb")]
            Self::File(transaction) => transaction
                .read()
                .await
                .map_err(native_error)?
                .execute(statements)
                .await
                .map_err(from_engine),
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Self::IndexedDb(transaction) => {
                transaction.execute(statements).await.map_err(from_engine)
            }
            #[cfg(feature = "in-memory")]
            Self::InMemory(transaction) => {
                transaction.execute(statements).await.map_err(from_engine)
            }
        }
    }

    pub(crate) async fn execute_translated<F>(
        &mut self,
        translation: F,
    ) -> SqlResult<Vec<QueryResult>>
    where
        F: Future<Output = SqlResult<Vec<Statement>>>,
    {
        match self {
            #[cfg(feature = "redb")]
            Self::File(transaction) => {
                let mut transaction = transaction.read().await.map_err(native_error)?;
                transaction
                    .execute(translation.await?)
                    .await
                    .map_err(from_engine)
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Self::IndexedDb(transaction) => transaction
                .execute(translation.await?)
                .await
                .map_err(from_engine),
            #[cfg(feature = "in-memory")]
            Self::InMemory(transaction) => transaction
                .execute(translation.await?)
                .await
                .map_err(from_engine),
        }
    }

    pub async fn commit(self) -> SqlResult<()> {
        match self {
            #[cfg(feature = "redb")]
            Self::File(transaction) => {
                let (_admitted, transaction) = transaction.take().await.map_err(native_error)?;
                transaction.commit().await.map_err(from_engine)
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Self::IndexedDb(transaction) => transaction.commit().await.map_err(from_engine),
            #[cfg(feature = "in-memory")]
            Self::InMemory(transaction) => transaction.commit().await.map_err(from_engine),
        }
    }

    pub async fn rollback(self) -> SqlResult<()> {
        match self {
            #[cfg(feature = "redb")]
            Self::File(transaction) => {
                let (_admitted, transaction) = transaction.take().await.map_err(native_error)?;
                transaction.rollback().await.map_err(from_engine)
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            Self::IndexedDb(transaction) => transaction.rollback().await.map_err(from_engine),
            #[cfg(feature = "in-memory")]
            Self::InMemory(transaction) => transaction.rollback().await.map_err(from_engine),
        }
    }
}

type SqlResult<T> = Result<T, QueryError>;

macro_rules! database_call {
    ($database:expr, |$engine:ident| $body:expr) => {{
        match &$database.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(resource) => {
                let admitted = resource.read().await.map_err(native_error)?;
                let $engine = &*admitted;
                let result = $body.map_err(from_engine);
                drop(admitted);
                result
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb($engine) => $body.map_err(from_engine),
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory($engine) => $body.map_err(from_engine),
        }
    }};
}

impl Database {
    /// Shared hosts permanently fence and remove only the selected namespace, never the host file.
    /// Dedicated files permanently fence issued handles and opens, drain admitted operations,
    /// cancels sessions, rolls back idle transactions, then removes this dedicated file.
    /// Native hosting is not managed: an owner that attempted native hosting returns Unsupported.
    /// Unix-only; callers must exclusively own the pathname and marker namespace.
    /// The permanent `.ofdb-retired` marker is retained, including after success.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-native backend or a lifecycle/filesystem failure.
    #[cfg(feature = "redb")]
    pub async fn retire_and_remove(&self) -> io::Result<()> {
        let result = match &self.backend {
            DatabaseBackend::File(resource) => resource.retire_and_remove().await,
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(_) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "not a native resource",
            )),
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(_) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
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
    ///
    /// # Errors
    ///
    /// Returns an error if the retired file cannot be removed or still has a live owner.
    #[cfg(feature = "redb")]
    pub fn remove_retired(path: impl AsRef<Path>) -> io::Result<()> {
        engine_redb::remove_retired(path)
    }

    /// Creates the sync-owned lifecycle for committed changes, including remote apply.
    /// `ChangeSubscription::events` returns a stream of committed changes.
    /// Merge with a KV subscription to reconcile both stores in one callback.
    /// Dropping the stream cancels its in-flight work.
    ///
    /// # Errors
    ///
    /// Returns an error if this database has no committed-change subscription.
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

    /// Interpret selected work before exporting state. Acknowledge only after successful sync.
    ///
    /// # Errors
    ///
    /// Returns an error if admission or invalidation decoding fails.
    #[cfg(feature = "sync")]
    pub async fn sync_invalidations(
        &self,
        work: &crate::ChangeWork,
    ) -> SqlResult<Vec<crate::SqlInvalidation>> {
        match &self.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(resource) => {
                let engine = resource.read().await.map_err(native_error)?;
                Box::pin(engine_automerge::committed_invalidations(&engine, work))
                    .await
                    .map_err(from_engine)
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                Box::pin(engine_automerge::committed_invalidations(engine, work))
                    .await
                    .map_err(from_engine)
            }
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => {
                Box::pin(engine_automerge::committed_invalidations(engine, work))
                    .await
                    .map_err(from_engine)
            }
        }
    }

    /// Opens origin-scoped browser persistence. Never falls back to memory.
    /// Transactions materialize the whole database; stale writes fail without retry.
    #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
    pub async fn open_indexeddb(name: &str) -> SqlResult<Self> {
        let kernel = IndexedDbKernel::open(name).await.map_err(from_engine)?;
        let engine = Engine::new(kernel, AutomergeRowCodec::new());
        #[cfg(feature = "sync")]
        let changes = engine.change_signal().clone();
        Ok(Self {
            backend: DatabaseBackend::IndexedDb(engine),
            #[cfg(feature = "sync")]
            changes: Some(changes),
        })
    }

    #[must_use]
    pub fn client(&self) -> crate::Client {
        crate::Client::embedded(self.clone())
    }

    /// Opens an isolated SQL catalog in a shared host. Retirement removes only this namespace.
    ///
    /// # Errors
    ///
    /// Returns an error if the namespace or native resource cannot be opened.
    #[cfg(feature = "redb")]
    pub fn open_shared(host: &engine_redb::SharedDatabase, name: &str) -> SqlResult<Self> {
        let namespace = host
            .namespace(&alloc::format!("sql:{name}"))
            .map_err(native_error)?;
        let kernel = RedbKernel::namespaced(namespace.clone());
        let resource = engine_redb::NativeResource::new_shared(
            namespace,
            Engine::new(kernel, AutomergeRowCodec::new()),
        )
        .map_err(native_error)?;
        #[cfg(feature = "sync")]
        let changes = resource
            .inspect(|engine| engine.change_signal().clone())
            .map_err(native_error)?;
        Ok(Self {
            backend: DatabaseBackend::File(resource),
            #[cfg(feature = "sync")]
            changes: Some(changes),
        })
    }

    /// Opens or initializes native persistence. Use `open_existing` for acknowledged resources.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    #[cfg(feature = "redb")]
    pub fn open(path: impl AsRef<Path>) -> SqlResult<Self> {
        Self::open_native(path.as_ref(), true)
    }

    /// Validates and opens existing native persistence without creating or initializing it.
    ///
    /// # Errors
    ///
    /// Returns an error if the existing database cannot be validated or opened.
    #[cfg(feature = "redb")]
    pub fn open_existing(path: impl AsRef<Path>) -> SqlResult<Self> {
        Self::open_native(path.as_ref(), false)
    }

    #[cfg(feature = "redb")]
    fn open_native(path: &Path, create: bool) -> SqlResult<Self> {
        let database = engine_redb::open_unretired(path, create).map_err(native_error)?;
        let database = engine_redb::RedbDatabase::new(Arc::new(database));
        let resource = engine_redb::NativeResource::new(
            path,
            Engine::new(
                RedbKernel::from_database(database),
                AutomergeRowCodec::new(),
            ),
        )
        .map_err(native_error)?;
        #[cfg(feature = "sync")]
        let changes = resource
            .inspect(|engine| engine.change_signal().clone())
            .map_err(native_error)?;
        Ok(Self {
            backend: DatabaseBackend::File(resource),
            #[cfg(feature = "sync")]
            changes: Some(changes),
        })
    }

    #[cfg(feature = "in-memory")]
    #[must_use]
    pub fn in_memory() -> Self {
        let kernel = InMemoryKernel::new();
        let engine = Engine::new(kernel, AutomergeRowCodec::new());
        Self {
            backend: DatabaseBackend::InMemory(engine),
            #[cfg(feature = "sync")]
            changes: None,
        }
    }

    pub(crate) async fn begin_transaction(&self) -> SqlResult<EmbeddedTransaction> {
        match &self.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(resource) => {
                let engine = resource.read().await.map_err(native_error)?;
                let transaction = engine.transaction().await.map_err(from_engine)?;
                Ok(EmbeddedTransaction::File(
                    resource.transaction(Box::new(transaction)),
                ))
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => engine
                .transaction()
                .await
                .map(EmbeddedTransaction::IndexedDb)
                .map_err(from_engine),
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => engine
                .transaction()
                .await
                .map(EmbeddedTransaction::InMemory)
                .map_err(from_engine),
        }
    }

    pub(crate) async fn query_row_conflicts(
        &self,
        table: &str,
        key: &Row,
    ) -> SqlResult<Vec<String>> {
        database_call!(self, |engine| conflict_stage(
            || engine.row_conflicts(table, key)
        )
        .await)
    }

    pub(crate) async fn query_row_conflicts_batch(
        &self,
        table: &str,
        keys: &[Row],
    ) -> SqlResult<Vec<Vec<String>>> {
        database_call!(self, |engine| conflict_stage(
            || engine.row_conflicts_batch(table, keys)
        )
        .await)
    }

    pub(crate) async fn query_row_conflict_values(
        &self,
        table: &str,
        key: &Row,
    ) -> SqlResult<Vec<(String, Vec<Value>)>> {
        database_call!(self, |engine| conflict_stage(
            || engine.row_conflict_values(table, key)
        )
        .await)
    }

    pub(crate) async fn query_resolve_row(
        &self,
        table: &str,
        key: &Row,
        values: Vec<(String, Value)>,
    ) -> SqlResult<()> {
        database_call!(self, |engine| conflict_stage(
            || engine.resolve_row(table, key, values)
        )
        .await)
    }

    pub(crate) async fn query_execute_untimed(
        &self,
        statements: Vec<Statement>,
    ) -> SqlResult<Vec<QueryResult>> {
        if statements.is_empty() {
            return Err(QueryError::new(
                crate::ErrorKind::Validation,
                "statement batch must not be empty",
            ));
        }
        database_call!(self, |engine| engine.execute(statements).await)
    }

    pub(crate) async fn query_translate_and_execute_with_params<T>(
        &self,
        query: &str,
        params: Option<&QueryParams>,
        translator: &T,
    ) -> SqlResult<Vec<QueryResult>>
    where
        T: Translator + platform::MaybeSync,
    {
        database_call!(self, |engine| {
            engine
                .translate_and_execute_with_params(query, params, translator)
                .await
        })
    }

    pub(crate) async fn query_translate_and_execute<T>(
        &self,
        query: &str,
        translator: &T,
    ) -> SqlResult<Vec<QueryResult>>
    where
        T: Translator + platform::MaybeSync,
    {
        database_call!(self, |engine| engine
            .translate_and_execute(query, translator)
            .await)
    }

    pub(crate) async fn query_translate_and_select<T, U>(
        &self,
        query: &str,
        translator: &T,
    ) -> SqlResult<Vec<U>>
    where
        T: Translator + platform::MaybeSync,
        U: FromRow,
    {
        database_call!(self, |engine| engine
            .translate_and_select(query, translator)
            .await)
    }

    /// Serves SQL requests over TCP until shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error if hosting or the server fails.
    #[cfg(feature = "server")]
    pub async fn serve_tcp(
        &self,
        address: SocketAddr,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.serve_tcp_with_tls(address, None, shutdown).await
    }

    /// Serves SQL requests over TCP with optional TLS until shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error if TLS configuration, hosting, or the server fails.
    #[cfg(feature = "server")]
    pub async fn serve_tcp_with_tls(
        &self,
        address: SocketAddr,
        identity: Option<(Vec<u8>, Vec<u8>)>,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        self.serve_tcp_with_mtls(address, identity, None, shutdown)
            .await
    }

    /// Serves SQL requests over TCP with optional mutual TLS until shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error if TLS configuration, hosting, or the server fails.
    #[cfg(feature = "server")]
    pub async fn serve_tcp_with_mtls(
        &self,
        address: SocketAddr,
        mut identity: Option<(Vec<u8>, Vec<u8>)>,
        client_ca: Option<Vec<u8>>,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        match &self.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(engine) => {
                engine
                    .hosting(async {
                        let engine = engine.hosting_backend().await?;
                        let server =
                            crate::Server::tcp(address, crate::EngineExecutor::new(engine.clone()));
                        drop(engine);
                        let server = match identity.take() {
                            Some((certificate, private_key)) => {
                                server.tls_identity(certificate, private_key)
                            }
                            None => server,
                        };
                        let server = match client_ca {
                            Some(ca) => server.tls_client_ca(ca),
                            None => server,
                        };
                        server.serve(shutdown).await
                    })
                    .await?
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                let server =
                    crate::Server::tcp(address, crate::EngineExecutor::new(engine.clone()));
                let server = match identity.take() {
                    Some((certificate, private_key)) => {
                        server.tls_identity(certificate, private_key)
                    }
                    None => server,
                };
                let server = match client_ca {
                    Some(ca) => server.tls_client_ca(ca),
                    None => server,
                };
                server.serve(shutdown).await
            }
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => {
                let server =
                    crate::Server::tcp(address, crate::EngineExecutor::new(engine.clone()));
                let server = match identity.take() {
                    Some((certificate, private_key)) => {
                        server.tls_identity(certificate, private_key)
                    }
                    None => server,
                };
                let server = match client_ca {
                    Some(ca) => server.tls_client_ca(ca),
                    None => server,
                };
                server.serve(shutdown).await
            }
        }
    }

    /// Serves SQL requests over a Unix socket until shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error if hosting or the server fails.
    #[cfg(all(feature = "server", unix))]
    pub async fn serve_unix(
        &self,
        path: impl Into<PathBuf> + Send,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), Box<dyn StdError + Send + Sync>> {
        match &self.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(engine) => {
                engine
                    .hosting(async {
                        let engine = engine.hosting_backend().await?;
                        crate::Server::unix(path, crate::EngineExecutor::new(engine.clone()))
                            .serve(shutdown)
                            .await
                    })
                    .await?
            }
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                crate::Server::unix(path, crate::EngineExecutor::new(engine.clone()))
                    .serve(shutdown)
                    .await
            }
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => {
                crate::Server::unix(path, crate::EngineExecutor::new(engine.clone()))
                    .serve(shutdown)
                    .await
            }
        }
    }

    /// Unchecked sync. Caller owns admission; use `synchronize_checked` for per-apply
    /// authorization/cancellation that can change during the session.
    #[cfg(feature = "sync")]
    /// Synchronization. Streams local changes to peer and applies remote changes.
    /// Uses the engine's change subscription for outbound events and applies inbound events via the reconciler.
    ///
    /// # Errors
    ///
    /// Returns an error if change subscription, transport, or remote application fails.
    #[cfg(feature = "sync")]
    pub async fn synchronize<T: SyncTransport>(
        &self,
        transport: T,
        config: PushSynchronizeConfig,
    ) -> Result<PushResult, PushSynchronizeError<T::Error>> {
        let subscription = self
            .sync_changes()
            .map_err(|e| PushSynchronizeError::Apply(e.to_string()))?;

        let backend = self.backend.clone();
        let apply_inbound = move |event: ChangeEvent| {
            let table = event.key.table.clone();
            let key_bytes = event.key.key.clone();
            let blob = event.blob;

            let backend = backend.clone();
            async move {
                let row_uuid = uuid::Uuid::from_slice(&key_bytes)
                    .map_err(|e| format!("Invalid row UUID: {e}"))?;
                let change = automerge::Change::from_bytes(blob.clone())
                    .map_err(|e| format!("Invalid change: {e}"))?;
                let change_hash = change.hash();
                let change_id = engine::DocumentChangeId(change_hash.0.to_vec());

                let backend = backend; // move ownership into async block
                let result: Result<(), String> = (async move {
                    match &backend {
                        #[cfg(feature = "redb")]
                        DatabaseBackend::File(resource) => {
                            let engine = resource.read().await.map_err(|e| e.to_string())?;
                            engine
                                .apply_inbound_change(&table, row_uuid, &change_id, &blob)
                                .await
                                .map_err(|e| e.to_string())?;
                            drop(engine);
                            Ok(())
                        }
                        #[cfg(all(
                            feature = "indexeddb",
                            target_arch = "wasm32",
                            target_os = "unknown"
                        ))]
                        DatabaseBackend::IndexedDb(engine) => {
                            engine
                                .apply_inbound_change(&table, row_uuid, &change_id, &blob)
                                .await
                                .map_err(|e| e.to_string())?;
                            Ok(())
                        }
                        #[cfg(feature = "in-memory")]
                        DatabaseBackend::InMemory(engine) => {
                            engine
                                .apply_inbound_change(&table, row_uuid, &change_id, &blob)
                                .await
                                .map_err(|e| e.to_string())?;
                            Ok(())
                        }
                    }
                })
                .await;
                result
            }
        };

        subscription
            .push_synchronize(transport, apply_inbound, config)
            .await
    }
}

#[cfg(feature = "sync")]
#[cfg(all(test, feature = "redb", feature = "sql"))]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use std::{
        env, fs, process,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn conflict_query_futures_keep_engine_state_indirect() {
        let directory = tempfile::tempdir().expect("database directory");
        let database =
            Database::open(directory.path().join("conflicts.redb")).expect("conflict database");
        let key = Row::new(Vec::new());
        let keys = [key.clone()];
        let sizes = [
            size_of_val(&database.query_row_conflicts("security", &key)),
            size_of_val(&database.query_row_conflicts_batch("security", &keys)),
            size_of_val(&database.query_row_conflict_values("security", &key)),
            size_of_val(&database.query_resolve_row("security", &key, Vec::new())),
        ];
        assert!(
            sizes.iter().all(|size| *size < 4096),
            "conflict dispatch must not embed engine futures: {sizes:?}"
        );
    }

    #[cfg(feature = "in-memory")]
    #[test]
    fn embedded_handles_reject_empty_statement_batches() {
        block_on(async {
            let database = Database::in_memory();
            let error = database
                .client()
                .execute(Vec::new())
                .await
                .expect_err("empty statement batches are invalid");
            assert_eq!(error.kind, crate::ErrorKind::Validation);
        });
    }

    #[cfg(feature = "in-memory")]
    #[test]
    fn in_memory_database_uses_the_engine_api() {
        block_on(async {
            let database = Database::in_memory();
            let client = database.client();
            client
                .execute_sql("CREATE TABLE users (id UUID PRIMARY KEY, name TEXT)", None)
                .await
                .expect("execute SQL text");
            client
                .execute_sql("SELECT id FROM users", None)
                .await
                .expect("query created table");
        });
    }

    #[test]
    fn file_database_reopens_persisted_schema() {
        block_on(async {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let database_path =
                env::temp_dir().join(format!("ofdb-database-{}-{nanos}.redb", process::id()));

            {
                let database = Database::open(&database_path).unwrap();
                database
                    .client()
                    .translate_and_execute(
                        "CREATE TABLE users (id UUID PRIMARY KEY, name TEXT)",
                        &crate::SqlTranslator,
                    )
                    .await
                    .unwrap()
            };

            let database = Database::open(&database_path).unwrap();
            database
                .client()
                .translate_and_execute("SELECT id FROM users", &crate::SqlTranslator)
                .await
                .expect("query persisted schema");
            let _ = fs::remove_file(&database_path);
        });
    }
}

#[cfg(feature = "redb")]
#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err supplies owned I/O errors"
)]
fn native_error(error: io::Error) -> QueryError {
    QueryError::new(crate::ErrorKind::Storage, error.to_string())
}

#[cfg(all(feature = "validated-history", not(target_arch = "wasm32")))]
#[path = "history_session.rs"]
mod history_session;

#[cfg(all(
    feature = "validated-history",
    target_arch = "wasm32",
    target_os = "unknown"
))]
#[path = "browser_history_session.rs"]
mod browser_history_session;
