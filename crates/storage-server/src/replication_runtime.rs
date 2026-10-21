use std::{collections::BTreeMap, io, path::PathBuf, sync::Arc, time::Duration};

use iroh::EndpointId;
use iroh_chain::Server;
use model::contract::SelectedResource;
use storage_service::{DatabaseRuntime, ScopedFileSystemRuntime};
use tokio::{sync::watch, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{
    LocalReplicationAuthorizer, data_protocol::DataProtocolHandler,
    database_protocol::DatabaseProtocolHandler, storage_protocol::StorageProtocolHandler,
};

pub struct StorageReplicationRuntime {
    data_handler: DataProtocolHandler,
    host: ReplicationHost,
    changes: watch::Sender<bool>,
    policy_task: Option<JoinHandle<()>>,
    sync_task: Option<JoinHandle<()>>,
    lifetime: CancellationToken,
    #[cfg(test)]
    catalog_wake: Option<crate::execution_test_support::Gate>,
}

#[derive(Clone)]
struct ReplicationHost {
    server: Server,
    authorizer: LocalReplicationAuthorizer,
    databases: Arc<DatabaseRuntime>,
    file_systems: Arc<ScopedFileSystemRuntime<EndpointId>>,
    database: DatabaseProtocolHandler,
    file_system: StorageProtocolHandler,
    #[cfg(test)]
    job_stopped: Arc<tokio::sync::Notify>,
}

type JobKey = (EndpointId, String, String, String, String, bool);

enum Job {
    Running(JoinHandle<bool>),
    TombstoneDelivered,
}

#[derive(Default)]
struct Jobs(BTreeMap<JobKey, Job>);

impl Jobs {
    async fn retain_peers(&mut self, server: &Server) {
        let removed = self
            .0
            .keys()
            .filter(|key| !server.peers().contains(key.0))
            .cloned()
            .collect::<Vec<_>>();
        for key in removed {
            if let Some(Job::Running(task)) = self.0.remove(&key) {
                task.abort();
                let _ = task.await;
            }
        }
    }

    async fn cancel(&mut self) {
        let tasks = std::mem::take(&mut self.0);
        for job in tasks.values() {
            if let Job::Running(task) = job {
                task.abort();
            }
        }
        for job in tasks.into_values() {
            if let Job::Running(task) = job {
                let _ = task.await;
            }
        }
    }
}

impl Drop for Jobs {
    fn drop(&mut self) {
        for job in self.0.values() {
            if let Job::Running(task) = job {
                task.abort();
            }
        }
    }
}

impl StorageReplicationRuntime {
    pub fn start(
        server: Server,
        authorizer: LocalReplicationAuthorizer,
        databases: Arc<DatabaseRuntime>,
        file_systems: Arc<ScopedFileSystemRuntime<EndpointId>>,
        staging_directory: PathBuf,
        cancellation_token: CancellationToken,
    ) -> Self {
        let lifetime = cancellation_token.child_token();
        let admission = crate::replication_admission::ReplicationAdmission::new(lifetime.clone());
        admission.fence();
        let mut database = DatabaseProtocolHandler::new(
            server.clone(),
            authorizer.clone(),
            databases.clone(),
            staging_directory,
            lifetime.clone(),
        );
        database.admission = admission.clone();
        let mut file_system = StorageProtocolHandler::new(
            server.clone(),
            authorizer.clone(),
            file_systems.clone(),
            lifetime.clone(),
        );
        file_system.admission = admission;
        file_system
            .install_passthrough_reader()
            .expect("new filesystem content-reader lock is unpoisoned");
        let data_handler =
            DataProtocolHandler::new(database.clone(), file_system.clone(), lifetime.clone());
        Self {
            data_handler,
            host: ReplicationHost {
                server,
                authorizer,
                databases,
                file_systems,
                database,
                file_system,
                #[cfg(test)]
                job_stopped: Arc::default(),
            },
            changes: watch::channel(true).0,
            policy_task: None,
            sync_task: None,
            lifetime,
            #[cfg(test)]
            catalog_wake: None,
        }
    }

    pub fn data_handler(&self) -> DataProtocolHandler {
        self.data_handler.clone()
    }

    /// Install the merged IdP/Management committed-change subscription before starting.
    pub fn set_policy_changes(&mut self, mut subscription: ofdb_sql::ChangeSubscription) {
        if let Some(task) = self.policy_task.take() {
            task.abort();
        }
        let changes = self.changes.clone();
        let admission = self.host.database.admission.clone();
        let lifetime = self.lifetime.clone();
        admission.fence();
        changes.send_replace(true);
        self.policy_task = Some(tokio::spawn(async move {
            loop {
                let changed = tokio::select! {
                    biased;
                    () = lifetime.cancelled() => break,
                    result = subscription.changed() => result,
                };
                admission.fence();
                if changed.is_err() {
                    break;
                }
                changes.send_modify(|_| {});
            }
            admission.fence();
            changes.send_replace(false);
        }));
    }

    pub fn start_sync(&mut self, cancellation_token: CancellationToken) -> io::Result<()> {
        if self.sync_task.is_some() {
            return Ok(());
        }
        if self.policy_task.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Storage replication requires set_policy_changes before start_background_tasks; remote Management needs an explicit notification bridge",
            ));
        }
        let lifetime = self.lifetime.clone();
        let host = self.host.clone();
        let mut changes = self.changes.subscribe();
        let mut databases = host.databases.catalog_changes();
        let mut filesystems = host.file_systems.catalog_changes();
        let mut peers = host.server.peers().subscribe_changes();
        #[cfg(test)]
        let mut catalog_wake = self.catalog_wake.clone();
        self.sync_task = Some(tokio::spawn(async move {
            let admission = host.database.admission.clone();
            let mut jobs = Jobs::default();
            let mut retry = Duration::from_millis(100);
            loop {
                if changes.has_changed().unwrap_or(true) {
                    jobs.cancel().await;
                    let _ = changes.borrow_and_update();
                }
                if !*changes.borrow() {
                    jobs.cancel().await;
                    return;
                }
                let generation = admission.generation();
                let mut configure = Box::pin(host.configure());
                let mut invalidated = false;
                let configured = loop {
                    tokio::select! {
                        biased;
                        () = lifetime.cancelled() => { jobs.cancel().await; return; },
                        () = cancellation_token.cancelled() => { lifetime.cancel(); jobs.cancel().await; return; },
                        result = changes.changed() => {
                            jobs.cancel().await;
                            if result.is_err() || !*changes.borrow() { return; }
                            invalidated = true;
                        }
                        result = peers.changed() => {
                            if result.is_err() { jobs.cancel().await; return; }
                            jobs.retain_peers(&host.server).await;
                            invalidated = true;
                        }
                        result = &mut configure => break result,
                    }
                };
                // Policy writes fence sessions immediately but never interrupt their own
                // cleanup pass. Only a pass from the current generation may admit jobs.
                if invalidated || generation != admission.generation() {
                    continue;
                }
                let failed = configured.is_err();
                match configured {
                    Err(error) => {
                        log::warn!("Storage replication configuration pending: {error}");
                        admission.fence();
                        jobs.cancel().await;
                    }
                    Ok(desired) => {
                        if !admission.open(generation) {
                            continue;
                        }
                        host.configure_jobs(&mut jobs, desired).await;
                        retry = Duration::from_millis(100);
                    }
                }
                tokio::select! {
                    biased;
                    () = lifetime.cancelled() => { jobs.cancel().await; return; },
                    () = cancellation_token.cancelled() => { lifetime.cancel(); jobs.cancel().await; return; },
                    result = changes.changed() => {
                        jobs.cancel().await;
                        if result.is_err() || !*changes.borrow() { return; }
                    }
                    result = peers.changed() => {
                        if result.is_err() { jobs.cancel().await; return; }
                        jobs.retain_peers(&host.server).await;
                    }
                    _ = databases.changed() => {
                        #[cfg(test)]
                        if let Some(gate) = catalog_wake.take() { gate.pause().await; }
                    }
                    _ = filesystems.changed() => {}
                    () = tokio::time::sleep(retry), if failed => {
                        retry = (retry * 2).min(Duration::from_secs(30));
                    }
                }
            }
        }));
        Ok(())
    }

    pub async fn shutdown(mut self) -> io::Result<()> {
        self.lifetime.cancel();
        if let Some(task) = self.policy_task.take() {
            let _ = task.await;
        }
        if let Some(task) = self.sync_task.take() {
            match task.await {
                Ok(()) => {}
                Err(error) if error.is_cancelled() => {}
                Err(error) => return Err(io::Error::other(error)),
            }
        }
        Ok(())
    }
}

impl ReplicationHost {
    async fn configure(&self) -> io::Result<BTreeMap<JobKey, SelectedResource>> {
        let local = self.server.endpoint().id();
        // This also completes pending owner cleanup even when the peer inventory is empty.
        let selected = self
            .authorizer
            .selected_resources(local)
            .await
            .map_err(io::Error::other)?;
        let databases = selected
            .iter()
            .filter(|resource| resource.kind == "database")
            .filter_map(|resource| {
                Some((
                    resource.owner_subject.clone(),
                    resource.application_id.parse().ok()?,
                    resource.resource_id.parse().ok()?,
                ))
            })
            .collect::<Vec<_>>();
        self.databases.reconcile_selected_copies(&databases).await?;
        let filesystems = selected
            .iter()
            .filter(|resource| resource.kind == "filesystem")
            .filter_map(|resource| {
                Some((
                    resource.owner_subject.clone(),
                    resource.application_id.parse().ok()?,
                    file_system::FileSystemId::parse(&resource.resource_id).ok()?,
                ))
            })
            .collect::<Vec<_>>();
        self.file_systems
            .remove_unselected_projected_resources(&filesystems)
            .await
            .map_err(io::Error::other)?;
        let mut desired = BTreeMap::new();
        for peer in self.server.peers().ids() {
            if peer == local {
                continue;
            }
            for resource in &selected {
                if !self.authorizer.authorize(local, peer, resource).await {
                    continue;
                }
                let scope = ResourceScope(resource);
                let deleted = if resource.kind == "database" {
                    self.databases.is_tombstoned(
                        &scope,
                        resource.resource_id.parse().map_err(io::Error::other)?,
                    )?
                } else {
                    self.file_systems
                        .is_tombstoned(
                            &scope,
                            file_system::FileSystemId::parse(&resource.resource_id)
                                .map_err(io::Error::other)?,
                        )
                        .await
                        .map_err(io::Error::other)?
                };
                if resource.kind == "filesystem"
                    && !deleted
                    && local.to_string() >= peer.to_string()
                {
                    continue;
                }
                let key = (
                    peer,
                    resource.owner_subject.clone(),
                    resource.application_id.clone(),
                    resource.kind.clone(),
                    resource.resource_id.clone(),
                    deleted,
                );
                desired.insert(key, resource.clone());
            }
        }
        Ok(desired)
    }

    async fn configure_jobs(&self, jobs: &mut Jobs, desired: BTreeMap<JobKey, SelectedResource>) {
        let obsolete = jobs
            .0
            .keys()
            .filter(|key| !desired.contains_key(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in obsolete {
            if let Some(Job::Running(task)) = jobs.0.remove(&key) {
                task.abort();
                let _ = task.await;
            }
        }
        for (key, resource) in desired {
            match jobs.0.get(&key) {
                Some(Job::TombstoneDelivered) => continue,
                Some(Job::Running(task)) if !task.is_finished() => continue,
                _ => {}
            }
            if let Some(Job::Running(task)) = jobs.0.remove(&key)
                && matches!(task.await, Ok(true))
                && key.5
            {
                jobs.0.insert(key, Job::TombstoneDelivered);
                continue;
            }
            let host = self.clone();
            let peer = key.0;
            let deleted = key.5;
            let task = tokio::spawn(async move {
                let admission = host.database.admission.clone();
                let delivered = admission
                    .run(
                        &host.server,
                        peer,
                        Box::pin(host.run_resource(peer, resource, deleted)),
                    )
                    .await
                    .unwrap_or(false);
                #[cfg(test)]
                host.job_stopped.notify_one();
                delivered
            });
            jobs.0.insert(key, Job::Running(task));
        }
    }

    async fn run_resource(
        &self,
        peer: EndpointId,
        resource: SelectedResource,
        deleted: bool,
    ) -> bool {
        let mut retry = Duration::from_millis(100);
        loop {
            let result = if resource.kind == "database" {
                let descriptor = crate::database_protocol::descriptor(&resource);
                if deleted {
                    self.database.synchronize_peer(peer, descriptor).await
                } else {
                    match self.database.sync_changes(&descriptor) {
                        Ok(Some(changes)) => {
                            changes
                                .run(|| async {
                                    let result = self
                                        .database
                                        .synchronize_peer(peer, descriptor.clone())
                                        .await;
                                    if let Err(error) = &result {
                                        log::debug!("database sync with {peer} pending: {error}");
                                    }
                                    result
                                })
                                .await;
                            return false;
                        }
                        Ok(None) => return false,
                        Err(error) => Err(error),
                    }
                }
            } else {
                self.file_system
                    .synchronize_peer(peer, crate::storage_protocol::descriptor(&resource))
                    .await
            };
            if result.is_ok() && deleted {
                return true;
            }
            if let Err(error) = result {
                log::debug!(
                    "Storage {} sync with {peer} pending: {error}",
                    resource.kind
                );
            }
            tokio::time::sleep(retry).await;
            retry = (retry * 2).min(Duration::from_secs(30));
        }
    }
}

struct ResourceScope<'a>(&'a SelectedResource);

impl storage_model::StorageNamespace for ResourceScope<'_> {
    fn user_sub(&self) -> &str {
        &self.0.owner_subject
    }
    fn application_id(&self) -> idp_model::model::Id {
        self.0
            .application_id
            .parse()
            .expect("authorizer validates application IDs")
    }
}

#[cfg(test)]
#[path = "replication_runtime_tests.rs"]
mod tests;

impl Drop for StorageReplicationRuntime {
    fn drop(&mut self) {
        self.lifetime.cancel();
        if let Some(task) = &self.policy_task {
            task.abort();
        }
        if let Some(task) = &self.sync_task {
            task.abort();
        }
    }
}
