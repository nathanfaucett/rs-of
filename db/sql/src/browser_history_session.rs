use core::fmt::Display;
use core::future::ready;

use history::{
    MemorySyncStageFactory, SessionConfig, SyncError, SyncResult, SyncRole, SyncTransport,
};

use super::{Database, DatabaseBackend};

impl Database {
    /// Exchanges SQL history v7 with bounded in-memory staging.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, protocol, staging, or engine failure.
    pub async fn exchange_resource_history_memory<T>(
        &self,
        transport: &mut T,
        config: &SessionConfig,
        role: SyncRole,
    ) -> Result<SyncResult, SyncError<T::Error>>
    where
        T: SyncTransport + platform::MaybeSend,
        T::Error: Display + platform::MaybeSend,
    {
        self.exchange_resource_history_memory_checked(transport, config, role, || Ok(()))
            .await
    }

    /// Checks the caller's fence before each history apply batch.
    /// On WASM, checks again before issuing the commit. Issued commits can finish.
    ///
    /// # Errors
    ///
    /// Returns an error for a rejected check or an exchange failure.
    pub async fn exchange_resource_history_memory_checked<T, Q>(
        &self,
        transport: &mut T,
        config: &SessionConfig,
        role: SyncRole,
        mut check: Q,
    ) -> Result<SyncResult, SyncError<T::Error>>
    where
        T: SyncTransport + platform::MaybeSend,
        T::Error: Display + platform::MaybeSend,
        Q: FnMut() -> Result<(), String> + platform::MaybeSend,
    {
        let factory = MemorySyncStageFactory::new(config.max_session_bytes);
        let check = || ready(check());
        match &self.backend {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                history::synchronize_with_stage_factory_checked(
                    engine, transport, config, role, &factory, check,
                )
                .await
            }
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => {
                history::synchronize_with_stage_factory_checked(
                    engine, transport, config, role, &factory, check,
                )
                .await
            }
            #[cfg(feature = "redb")]
            DatabaseBackend::File(_) => Err(SyncError::Staging(
                "native history staging requires a filesystem".into(),
            )),
        }
    }
}
