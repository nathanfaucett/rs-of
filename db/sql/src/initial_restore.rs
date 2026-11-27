use core::fmt::Display;

use engine::{Engine, InMemoryKernel, Kernel};
use engine_automerge::AutomergeRowCodec;
use history::{
    MemorySyncStageFactory, SessionConfig, SyncError, SyncRole, SyncRowCodec, SyncTransport,
};
use platform::MaybeSend;

use super::{Database, DatabaseBackend};
use crate::error::from_engine;

impl Database {
    /// Returns whether the local database has no retained catalog history. Restore checks again
    /// inside its publish transaction; this result is only a preflight.
    ///
    /// # Errors
    ///
    /// Returns an error if the catalog cannot be read or the backend is unsupported.
    pub async fn initial_restore_is_eligible(&self) -> Result<bool, crate::Error> {
        match &self.backend {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                history::catalog_history_is_empty_for(engine).await
            }
            DatabaseBackend::InMemory(engine) => {
                history::catalog_history_is_empty_for(engine).await
            }
            #[cfg(feature = "redb")]
            DatabaseBackend::File(_) => {
                Err(engine::EngineError::Unsupported("local initial restore"))
            }
        }
        .map_err(from_engine)
    }

    /// Reads the local-only replica tag, which may have been saved by a restore.
    /// # Errors
    /// Returns an error on a storage read failure or unsupported backend.
    pub async fn initial_restore_tag(&self) -> Result<Option<Vec<u8>>, crate::Error> {
        match &self.backend {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => history::initial_restore_tag_for(engine).await,
            DatabaseBackend::InMemory(engine) => history::initial_restore_tag_for(engine).await,
            #[cfg(feature = "redb")]
            DatabaseBackend::File(_) => {
                Err(engine::EngineError::Unsupported("local initial restore"))
            }
        }
        .map_err(from_engine)
    }

    /// Replaces the local-only replica tag if its current bytes match `expected`.
    /// The tag is not exported with SQL history.
    ///
    /// # Errors
    /// Returns an error on storage failure, concurrent writes, or an unsupported backend.
    pub async fn replace_initial_restore_tag(
        &self,
        expected: Option<Vec<u8>>,
        tag: Vec<u8>,
    ) -> Result<bool, crate::Error> {
        match &self.backend {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                Box::pin(history::replace_initial_restore_tag_for(
                    engine, expected, tag,
                ))
                .await
            }
            DatabaseBackend::InMemory(engine) => {
                Box::pin(history::replace_initial_restore_tag_for(
                    engine, expected, tag,
                ))
                .await
            }
            #[cfg(feature = "redb")]
            DatabaseBackend::File(_) => Err(engine::EngineError::Unsupported("local replica tag")),
        }
        .map_err(from_engine)
    }

    /// Pulls into a transient stage, then atomically publishes only into a pristine local database.
    /// Only revision-checked backends are supported. `check` may cancel before commit.
    ///
    /// # Errors
    ///
    /// Returns an error for existing local history, transport, validation, cancellation, or publish failure.
    pub async fn restore_resource_history_memory<T, Q>(
        &self,
        transport: &mut T,
        config: &SessionConfig,
        tag: Vec<u8>,
        check: Q,
    ) -> Result<(), SyncError<T::Error>>
    where
        T: SyncTransport + MaybeSend,
        T::Error: Display + MaybeSend,
        Q: FnOnce() -> Result<(), String> + MaybeSend,
    {
        match &self.backend {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                restore(engine, transport, config, tag, check).await
            }
            DatabaseBackend::InMemory(engine) => {
                restore(engine, transport, config, tag, check).await
            }
            #[cfg(feature = "redb")]
            DatabaseBackend::File(_) => {
                Err(engine::EngineError::Unsupported("local initial restore").into())
            }
        }
    }
}

async fn restore<K, R, T, Q>(
    engine: &Engine<K, R>,
    transport: &mut T,
    config: &SessionConfig,
    tag: Vec<u8>,
    check: Q,
) -> Result<(), SyncError<T::Error>>
where
    K: Kernel,
    R: SyncRowCodec<K::Transaction>,
    T: SyncTransport + MaybeSend,
    T::Error: Display + MaybeSend,
    Q: FnOnce() -> Result<(), String> + MaybeSend,
{
    if !history::catalog_history_is_empty_for(engine).await? {
        return Err(SyncError::DestinationHasHistory);
    }
    let stage = Engine::new(InMemoryKernel::new(), AutomergeRowCodec::new());
    let factory = MemorySyncStageFactory::new(config.max_session_bytes);
    Box::pin(history::synchronize_pull_with_stage_factory(
        &stage,
        transport,
        config,
        SyncRole::Initiator,
        &factory,
    ))
    .await?;
    Box::pin(history::restore_initial_history_for(
        engine,
        &stage,
        config.max_session_bytes,
        tag,
        check,
    ))
    .await
}
