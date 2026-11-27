use core::{fmt::Display, pin::Pin};
use std::path::Path;

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};

use super::{Database, DatabaseBackend};
#[cfg(feature = "redb")]
use crate::database::native_error;
use crate::error::from_engine;
use history::{SessionConfig, SyncError, SyncResult, SyncRole, SyncTransport};
use value::Value;
#[cfg(feature = "redb")]
type NativeStageFactory = history::NativeSyncStageFactory;
#[cfg(not(feature = "redb"))]
type NativeStageFactory = history::FileSyncStageFactory;

#[inline(never)]
fn sync_stage<F: Future>(make: impl FnOnce() -> F) -> Pin<Box<F>> {
    Box::pin(make())
}

impl Database {
    /// Returns the synchronization manifest for this database.
    ///
    /// # Errors
    ///
    /// Returns an error if native admission or manifest construction fails.
    pub async fn history_manifest(&self) -> Result<history::SyncManifest, crate::Error> {
        match &self.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(resource) => {
                let guard = resource.read().await.map_err(native_error)?;
                let engine = &*guard;
                let result = Box::pin(history::sync_manifest_for(engine))
                    .await
                    .map_err(from_engine);
                drop(guard);
                result
            }
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => Box::pin(history::sync_manifest_for(engine))
                .await
                .map_err(from_engine),
        }
    }
    /// Exports all synchronization state.
    ///
    /// # Errors
    ///
    /// Returns an error if native admission or state export fails.
    pub async fn export_history(&self) -> Result<Vec<history::SyncStateUnit>, crate::Error> {
        match &self.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(resource) => {
                let guard = resource.read().await.map_err(native_error)?;
                let engine = &*guard;
                let result = history::export_sync_state_for(engine)
                    .await
                    .map_err(from_engine);
                drop(guard);
                result
            }
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => history::export_sync_state_for(engine)
                .await
                .map_err(from_engine),
        }
    }
    /// Trusted native restore. The caller must authorize the payload; network owners must use checked exchange.
    ///
    /// # Errors
    ///
    /// Returns an error if native admission or state application fails.
    pub async fn apply_history(&self, unit: history::SyncStateUnit) -> Result<(), crate::Error> {
        match &self.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(resource) => {
                let guard = resource.read().await.map_err(native_error)?;
                let engine = &*guard;
                let result = Box::pin(history::apply_sync_state_for(engine, unit))
                    .await
                    .map_err(from_engine);
                drop(guard);
                result
            }
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => {
                Box::pin(history::apply_sync_state_for(engine, unit))
                    .await
                    .map_err(from_engine)
            }
        }
    }
    /// Imports trusted catalog and row history in separate validated batches.
    /// A later batch can fail after an earlier batch commits. Use a private stage if
    /// a failure must leave the destination unchanged. This is not peer admission.
    ///
    /// # Errors
    ///
    /// Returns an error if native admission or batch state application fails.
    pub async fn import_history(
        &self,
        units: Vec<history::SyncStateUnit>,
    ) -> Result<(), crate::Error> {
        match &self.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(resource) => {
                let guard = resource.read().await.map_err(native_error)?;
                let engine = &*guard;
                let result = Box::pin(history::apply_sync_state_batch_for(engine, units))
                    .await
                    .map_err(from_engine);
                drop(guard);
                result
            }
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => {
                Box::pin(history::apply_sync_state_batch_for(engine, units))
                    .await
                    .map_err(from_engine)
            }
        }
    }
    /// Exchanges history for one resource with admission checks.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, validation, staging, or synchronization failure.
    pub async fn exchange_resource_history<T, Q, U>(
        &self,
        transport: &mut T,
        config: &SessionConfig,
        role: SyncRole,
        directory: impl AsRef<Path>,
        check: Q,
    ) -> Result<SyncResult, SyncError<T::Error>>
    where
        T: SyncTransport + platform::MaybeSend,
        T::Error: Display + platform::MaybeSend,
        Q: FnMut() -> U + platform::MaybeSend,
        U: Future<Output = Result<(), String>> + platform::MaybeSend,
    {
        let factory = self.native_stage_factory(directory);
        match &self.backend {
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => {
                history::synchronize_with_stage_factory_checked(
                    engine, transport, config, role, &factory, check,
                )
                .await
            }
            #[cfg(feature = "redb")]
            DatabaseBackend::File(resource) => resource
                .session(sync_stage(|| async {
                    let engine = resource
                        .read()
                        .await
                        .map_err(|error| SyncError::PreApply(error.to_string()))?;
                    history::synchronize_with_stage_factory_checked(
                        &*engine, transport, config, role, &factory, check,
                    )
                    .await
                }))
                .await
                .map_err(|error| SyncError::PreApply(error.to_string()))?,
        }
    }
    /// Serves one SQL v7 history snapshot without applying any frames to this database.
    /// The transport must authorize every read and send.
    /// # Errors
    /// Returns an error for transport, protocol, storage, or native admission failure.
    #[cfg(feature = "redb")]
    pub async fn serve_resource_history<T>(
        &self,
        transport: &mut T,
        config: &SessionConfig,
        directory: impl AsRef<Path>,
    ) -> Result<SyncResult, SyncError<T::Error>>
    where
        T: SyncTransport + platform::MaybeSend,
        T::Error: Display + platform::MaybeSend,
    {
        let DatabaseBackend::File(resource) = &self.backend else {
            return Err(engine::EngineError::Unsupported("native history serving").into());
        };
        let factory = self.native_stage_factory(directory);
        resource
            .session(sync_stage(|| async {
                let engine = resource
                    .read()
                    .await
                    .map_err(|error| SyncError::PreApply(error.to_string()))?;
                let units = sync_stage(|| {
                    history::export_sync_state_bounded_for(&*engine, config.max_session_bytes)
                })
                .await?;
                drop(engine);
                let snapshot = engine::Engine::new(
                    engine::InMemoryKernel::new(),
                    engine_automerge::AutomergeRowCodec::new(),
                );
                sync_stage(|| history::apply_sync_state_batch_for(&snapshot, units)).await?;
                history::synchronize_pull_with_stage_factory(
                    &snapshot,
                    transport,
                    config,
                    SyncRole::Responder,
                    &factory,
                )
                .await
            }))
            .await
            .map_err(|error| SyncError::PreApply(error.to_string()))?
    }

    /// Checks first-upload eligibility. This is only a preflight; publish checks again atomically.
    ///
    /// # Errors
    ///
    /// Returns an error if native admission or the catalog read fails.
    #[cfg(feature = "redb")]
    pub async fn first_upload_is_eligible(&self) -> Result<bool, crate::Error> {
        let DatabaseBackend::File(resource) = &self.backend else {
            return Err(from_engine(engine::EngineError::Unsupported(
                "native first upload",
            )));
        };
        let engine = resource.read().await.map_err(native_error)?;
        sync_stage(|| history::catalog_history_is_empty_for(&*engine))
            .await
            .map_err(from_engine)
    }

    /// Receives SQL v7 into a private staging database and publishes it only into a previously
    /// unused native destination. A caller must report success separately from SQL v7 completion.
    /// The final check runs under the actual writer. It must be bounded and read-only on that host.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, validation, authorization, staging, or publish failure.
    #[cfg(feature = "redb")]
    pub async fn receive_first_upload<T, Q, U, P, V>(
        &self,
        transport: &mut T,
        config: &SessionConfig,
        directory: impl AsRef<Path>,
        mut check: Q,
        publish: P,
    ) -> Result<(), SyncError<T::Error>>
    where
        T: SyncTransport + platform::MaybeSend,
        T::Error: Display + platform::MaybeSend,
        Q: FnMut() -> U + platform::MaybeSend,
        U: Future<Output = Result<(), String>> + platform::MaybeSend,
        P: FnOnce() -> V + platform::MaybeSend,
        V: Future<Output = Result<(), String>> + platform::MaybeSend,
    {
        let DatabaseBackend::File(resource) = &self.backend else {
            return Err(engine::EngineError::Unsupported("native first upload").into());
        };
        let factory = self.native_stage_factory(directory);
        resource
            .session(sync_stage(|| async {
                let staged = engine::Engine::new(
                    engine::InMemoryKernel::new(),
                    engine_automerge::AutomergeRowCodec::new(),
                );
                sync_stage(|| {
                    history::synchronize_with_stage_factory_checked(
                        &staged,
                        transport,
                        config,
                        SyncRole::Responder,
                        &factory,
                        &mut check,
                    )
                })
                .await?;
                check().await.map_err(SyncError::PreApply)?;
                let engine = resource
                    .read()
                    .await
                    .map_err(|error| SyncError::PreApply(error.to_string()))?;
                sync_stage(|| {
                    history::publish_initial_history_for(
                        &*engine,
                        &staged,
                        config.max_session_bytes,
                        publish,
                    )
                })
                .await
            }))
            .await
            .map_err(|error| SyncError::PreApply(error.to_string()))?
    }

    fn native_stage_factory(&self, directory: impl AsRef<Path>) -> NativeStageFactory {
        let factory = history::FileSyncStageFactory::new(directory);
        match &self.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(resource) => {
                if let Some(namespace) = resource.shared_namespace() {
                    return history::NativeSyncStageFactory::shared(namespace);
                }
                #[cfg(unix)]
                {
                    let tracker = resource.stage_tracker();
                    let closed = tracker.clone();
                    factory
                        .with_file_tracking(
                            move |path| tracker.track(path),
                            move |path| closed.forget(path),
                        )
                        .into()
                }
                #[cfg(not(unix))]
                {
                    let _ = resource;
                    factory.into()
                }
            }
            #[cfg(all(feature = "in-memory", feature = "redb"))]
            DatabaseBackend::InMemory(_) => factory.into(),
            #[cfg(all(feature = "in-memory", not(feature = "redb")))]
            DatabaseBackend::InMemory(_) => factory,
        }
    }
    /// Exchanges validated table history with admission checks.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, validation, staging, or synchronization failure.
    pub async fn exchange_history<T, V, C, Q, U>(
        &self,
        transport: &mut T,
        config: &SessionConfig,
        role: SyncRole,
        directory: impl AsRef<Path>,
        tables: &[&str],
        validate: V,
        validate_candidate: C,
        check: Q,
    ) -> Result<SyncResult, SyncError<T::Error>>
    where
        T: SyncTransport + platform::MaybeSend,
        T::Error: Display + platform::MaybeSend,
        V: Fn(&str, &str, &Value) -> Result<(), String> + platform::MaybeSend + platform::MaybeSync,
        C: Fn(&history::SyncCandidateView) -> Result<(), String> + Send + Sync,
        Q: FnMut() -> U + platform::MaybeSend,
        U: Future<Output = Result<(), String>> + platform::MaybeSend,
    {
        let factory = self.native_stage_factory(directory);
        let factory = &factory;
        match &self.backend {
            #[cfg(feature = "redb")]
            DatabaseBackend::File(engine) => engine
                .session(sync_stage(|| async {
                    let engine = engine
                        .read()
                        .await
                        .map_err(|error| SyncError::PreApply(error.to_string()))?;

                    sync_stage(|| {
                        history::synchronize_tables_with_stage_factory_candidate_validated_checked(
                            &*engine,
                            transport,
                            config,
                            role,
                            factory,
                            tables,
                            validate,
                            &validate_candidate,
                            check,
                        )
                    })
                    .await
                }))
                .await
                .map_err(|error| SyncError::PreApply(error.to_string()))?,
            #[cfg(all(feature = "indexeddb", target_arch = "wasm32", target_os = "unknown"))]
            DatabaseBackend::IndexedDb(engine) => {
                sync_stage(|| {
                    history::synchronize_tables_with_stage_factory_candidate_validated_checked(
                        engine,
                        transport,
                        config,
                        role,
                        factory,
                        tables,
                        validate,
                        &validate_candidate,
                        check,
                    )
                })
                .await
            }
            #[cfg(feature = "in-memory")]
            DatabaseBackend::InMemory(engine) => {
                sync_stage(|| {
                    history::synchronize_tables_with_stage_factory_candidate_validated_checked(
                        engine,
                        transport,
                        config,
                        role,
                        factory,
                        tables,
                        validate,
                        &validate_candidate,
                        check,
                    )
                })
                .await
            }
        }
    }
}
