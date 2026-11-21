use core::fmt::Display;

use history::{SessionConfig, SyncError, SyncResult, SyncRole, SyncTransport};

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
        match &self.backend {
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                history::synchronize(engine, transport, config, role).await
            }
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => {
                history::synchronize(engine, transport, config, role).await
            }
            #[cfg(feature = "redb")]
            DatabaseBackend::File(_) => Err(SyncError::Staging(
                "native history staging requires a filesystem".into(),
            )),
        }
    }
}
