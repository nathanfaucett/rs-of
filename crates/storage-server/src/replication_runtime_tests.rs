use std::{
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use idp_model::model::Id;
use iroh::{Endpoint, address_lookup::MemoryLookup, endpoint::presets, protocol::Router};
use iroh_chain::{DATA_ALPN, EndpointIdStore, Server};
use management_service::DeviceState;
use model::contract::SelectedResource;

use storage_service::{DatabaseRuntime, ScopedFileSystemRuntime};
use tokio_util::sync::CancellationToken;

use super::{ResourceScope, StorageReplicationRuntime};
use crate::{
    DeviceReplicationPolicy, LocalReplicationAuthorizer,
    execution_test_support::{TestDirectory, sql},
};

struct Fixture {
    runtimes: Vec<StorageReplicationRuntime>,
    servers: Vec<Server>,
    routers: Vec<Router>,
    accepting: Vec<Arc<AtomicBool>>,
    rejected_connections: Vec<Arc<AtomicUsize>>,
    databases: Vec<Arc<DatabaseRuntime>>,
    filesystems: Vec<Arc<ScopedFileSystemRuntime<iroh::EndpointId>>>,
    selected: Arc<Mutex<Vec<SelectedResource>>>,
    allowed: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
    policy: Arc<ofdb_sql::Database>,
    cancellation: CancellationToken,
    owner: String,
    application: Id,
    _directory: TestDirectory,
}

impl Fixture {
    async fn new() -> Self {
        let _ = env_logger::try_init();
        let directory = TestDirectory::new();
        let lookup = MemoryLookup::new();
        let mut endpoints = Vec::new();
        for _ in 0..2 {
            endpoints.push(
                Endpoint::builder(presets::Minimal)
                    .address_lookup(lookup.clone())
                    .bind()
                    .await
                    .expect("endpoint"),
            );
        }
        endpoints.sort_by_key(|endpoint| endpoint.id().to_string());
        for endpoint in &endpoints {
            lookup.add_endpoint_info(endpoint.addr());
        }
        let ids = [endpoints[0].id(), endpoints[1].id()];
        let devices = [Id::now_v7(), Id::now_v7()];
        let owner = Id::now_v7().to_string();
        let application = Id::now_v7();
        let selected = Arc::new(Mutex::new(Vec::<SelectedResource>::new()));
        let allowed = Arc::new(AtomicBool::new(true));
        let calls = Arc::new(AtomicUsize::new(0));
        let policy = Arc::new(
            ofdb_sql::Database::open(directory.0.join("policy.redb")).expect("policy database"),
        );
        sql(&policy, "CREATE TABLE events (id UUID PRIMARY KEY)").await;
        let cancellation = CancellationToken::new();
        let mut runtimes = Vec::new();
        let mut servers = Vec::new();
        let mut routers = Vec::new();
        let mut accepting = Vec::new();
        let mut rejected_connections = Vec::new();
        let mut databases = Vec::new();
        let mut filesystems = Vec::new();
        for (index, endpoint) in endpoints.into_iter().enumerate() {
            let server = Server::new(endpoint.clone(), EndpointIdStore::default());
            let selected = selected.clone();
            let allowed = allowed.clone();
            let calls = calls.clone();
            let owner = owner.clone();
            let authorizer = LocalReplicationAuthorizer::new(move |endpoint| {
                calls.fetch_add(1, Ordering::SeqCst);
                let resources = selected.lock().expect("selection").clone();
                let owner = owner.clone();
                let allowed = allowed.load(Ordering::SeqCst);
                async move {
                    let side = ids
                        .iter()
                        .position(|id| *id == endpoint)
                        .ok_or("unknown endpoint")?;
                    Ok(DeviceReplicationPolicy {
                        device_id: devices[side],
                        endpoint_id: endpoint,
                        owner_subject: owner,
                        state: DeviceState::Approved,
                        owner_active: allowed,
                        admin_allowed: allowed,
                        resources,
                    })
                }
            });
            let db = Arc::new(
                DatabaseRuntime::new(directory.0.join(format!("db-{index}")))
                    .expect("database runtime"),
            );
            let fs = Arc::new(
                ScopedFileSystemRuntime::new(
                    directory.0.join(format!("fs-{index}")),
                    endpoint.id(),
                )
                .expect("filesystem runtime"),
            );
            let mut runtime = StorageReplicationRuntime::start(
                server.clone(),
                authorizer,
                db.clone(),
                fs.clone(),
                directory.0.join(format!("stage-{index}")),
                cancellation.clone(),
            );
            runtime.set_policy_changes(policy.sync_changes().expect("policy subscription"));
            let enabled = Arc::new(AtomicBool::new(true));
            let rejected = Arc::new(AtomicUsize::new(0));
            routers.push(
                Router::builder(endpoint)
                    .accept(
                        DATA_ALPN,
                        AcceptSwitch {
                            handler: runtime.data_handler(),
                            enabled: enabled.clone(),
                            rejected: rejected.clone(),
                        },
                    )
                    .spawn(),
            );
            accepting.push(enabled);
            rejected_connections.push(rejected);
            runtimes.push(runtime);
            servers.push(server);
            databases.push(db);
            filesystems.push(fs);
        }
        Self {
            runtimes,
            servers,
            routers,
            accepting,
            rejected_connections,
            databases,
            filesystems,
            selected,
            allowed,
            calls,
            policy,
            cancellation,
            owner,
            application,
            _directory: directory,
        }
    }

    fn resource(&self, kind: &str, id: Id) -> SelectedResource {
        SelectedResource {
            owner_subject: self.owner.clone(),
            application_id: self.application.to_string(),
            kind: kind.into(),
            resource_id: id.to_string(),
        }
    }

    async fn database(
        &self,
    ) -> (
        SelectedResource,
        [Arc<ofdb_sql::Database>; 2],
        [Arc<ofdb_kv::Database>; 2],
    ) {
        let resource = self.resource("database", Id::now_v7());
        let scope = ResourceScope(&resource);
        let id = resource.resource_id.parse().expect("database ID");
        let sql = [
            self.databases[0]
                .open_selected(&scope, id)
                .expect("SQL")
                .expect("selected SQL"),
            self.databases[1]
                .open_selected(&scope, id)
                .expect("SQL")
                .expect("selected SQL"),
        ];
        let kv = [
            self.databases[0]
                .open_kv_selected(&scope, id)
                .expect("KV")
                .expect("selected KV"),
            self.databases[1]
                .open_kv_selected(&scope, id)
                .expect("KV")
                .expect("selected KV"),
        ];
        self.selected
            .lock()
            .expect("selection")
            .push(resource.clone());
        (resource, sql, kv)
    }

    async fn filesystem(
        &self,
    ) -> (
        SelectedResource,
        [Arc<file_system::FileSystem<iroh::EndpointId>>; 2],
    ) {
        let placeholder = self.resource("filesystem", Id::now_v7());
        let scope = ResourceScope(&placeholder);
        let resource = self.filesystems[0]
            .create_resource(&scope, None)
            .await
            .expect("create filesystem");
        self.filesystems[1]
            .register_selected_resource(&scope, resource.id)
            .await
            .expect("selected filesystem");
        let files = [
            self.filesystems[0]
                .open_resource(&scope, resource.id)
                .await
                .expect("left filesystem"),
            self.filesystems[1]
                .open_resource(&scope, resource.id)
                .await
                .expect("right filesystem"),
        ];
        let resource = self.resource("filesystem", resource.id.as_uuid());
        self.selected
            .lock()
            .expect("selection")
            .push(resource.clone());
        (resource, files)
    }

    fn peers(&self, enabled: bool) {
        for side in 0..2 {
            self.servers[side].peers().replace(if enabled {
                vec![self.servers[1 - side].endpoint().id()]
            } else {
                vec![]
            });
        }
    }

    fn start(&mut self) {
        for runtime in &mut self.runtimes {
            runtime
                .start_sync(self.cancellation.clone())
                .expect("start with policy feed");
        }
    }

    async fn policy_changed(&self) {
        sql(
            &self.policy,
            &format!(
                "INSERT INTO events VALUES (CAST('{}' AS UUID))",
                Id::now_v7()
            ),
        )
        .await;
    }

    async fn close(self) {
        self.cancellation.cancel();
        for runtime in self.runtimes {
            runtime.shutdown().await.expect("replication shutdown");
        }
        for router in self.routers {
            router.shutdown().await.expect("router shutdown");
        }
    }
}

async fn until<F, Fut>(mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    tokio::time::timeout(Duration::from_secs(15), async {
        while !condition().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("replication makes progress without inventory polling");
}

async fn has_row(database: &ofdb_sql::Database, value: &str) -> bool {
    database
        .client()
        .translate_and_execute("SELECT value FROM documents", &db::SqlTranslator)
        .await
        .is_ok_and(|results| {
            results[0]
                .rows
                .iter()
                .any(|row| row.values[0] == db::Value::Text(value.into()))
        })
}

#[test]
fn database_changes_from_higher_id_and_new_peers_catch_up_then_stay_idle() {
    on_two_mib_workers(async {
        let mut fixture = Fixture::new().await;
        let (_, databases, kv) = fixture.database().await;
        fixture.start();
        sql(&databases[1], &format!("CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents VALUES (CAST('{}' AS UUID), 'initial')", Id::now_v7())).await;
        fixture.peers(true);
        until(|| has_row(&databases[0], "initial")).await;
        sql(
            &databases[1],
            &format!(
                "INSERT INTO documents VALUES (CAST('{}' AS UUID), 'higher')",
                Id::now_v7()
            ),
        )
        .await;
        until(|| has_row(&databases[0], "higher")).await;
        kv[1]
            .client()
            .set("changed", ofdb_kv::Value::Integer(42), None)
            .await
            .expect("local KV commit");
        until(|| async {
            kv[0].client().get("changed").await.expect("read KV")
                == Some(ofdb_kv::Value::Integer(42))
        })
        .await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let calls = fixture.calls.load(Ordering::SeqCst);
        fixture.peers(true);
        tokio::time::sleep(Duration::from_secs(11)).await;
        assert_eq!(
            fixture.calls.load(Ordering::SeqCst),
            calls,
            "idle replication does not call the authorizer/database path every ten seconds"
        );
        fixture.close().await;
    });
}

#[tokio::test]
async fn failed_job_reconnects_without_a_new_write_or_inventory_notification() {
    let mut fixture = Fixture::new().await;
    let (_, databases, _) = fixture.database().await;
    sql(&databases[1], &format!("CREATE TABLE documents (id UUID PRIMARY KEY, value TEXT); INSERT INTO documents VALUES (CAST('{}' AS UUID), 'retry')", Id::now_v7())).await;
    for accepting in &fixture.accepting {
        accepting.store(false, Ordering::SeqCst);
    }
    fixture.peers(true);
    fixture.start();
    until(|| async {
        fixture
            .rejected_connections
            .iter()
            .all(|rejected| rejected.load(Ordering::SeqCst) > 0)
    })
    .await;
    assert!(
        fixture
            .runtimes
            .iter()
            .all(|runtime| runtime.host.database.admission.allows()),
        "both lifecycles must admit replication before testing a transport retry"
    );
    assert!(!has_row(&databases[0], "retry").await);
    // Only the higher-ID worker can reconnect: the lower-ID initiator's target stays offline.
    fixture.accepting[0].store(true, Ordering::SeqCst);
    until(|| has_row(&databases[0], "retry")).await;
    // Removing the peer cancels its subscription. A later write stays local until re-admission.
    fixture.peers(false);
    tokio::time::sleep(Duration::from_millis(100)).await;
    sql(
        &databases[1],
        &format!(
            "INSERT INTO documents VALUES (CAST('{}' AS UUID), 'rejoined')",
            Id::now_v7()
        ),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!has_row(&databases[0], "rejoined").await);
    fixture.peers(true);
    until(|| has_row(&databases[0], "rejoined")).await;
    fixture.close().await;
}

#[tokio::test]
async fn policy_deselection_cleans_local_copies_without_peers_and_reselection_is_fresh() {
    let mut fixture = Fixture::new().await;
    let (resource, databases, _) = fixture.database().await;
    fixture.start();
    until(|| async { fixture.calls.load(Ordering::SeqCst) >= 2 }).await;
    fixture.selected.lock().expect("selection").clear();
    fixture.policy_changed().await;
    let scope = ResourceScope(&resource);
    let id = resource.resource_id.parse().expect("database ID");
    until(|| async {
        fixture
            .databases
            .iter()
            .all(|runtime| runtime.require_local_copy(&scope, id).is_err())
    })
    .await;
    let old = databases[0]
        .sync_changes()
        .expect("old generation subscription");
    let mut old = old;
    tokio::time::timeout(Duration::from_secs(5), async {
        while old.changed().await.is_ok() {}
    })
    .await
    .expect("old store generation retired");
    fixture
        .selected
        .lock()
        .expect("selection")
        .push(resource.clone());
    fixture.policy_changed().await;
    until(|| async {
        fixture
            .databases
            .iter()
            .all(|runtime| runtime.require_local_copy(&scope, id).is_ok())
    })
    .await;
    let fresh = fixture.databases[0]
        .open_selected(&scope, id)
        .expect("new generation")
        .expect("selected");
    assert!(!Arc::ptr_eq(&databases[0], &fresh));
    fixture.close().await;
}

#[tokio::test]
async fn filesystem_resources_have_independent_persistent_sessions() {
    let mut fixture = Fixture::new().await;
    let mut pairs = Vec::new();
    for _ in 0..2 {
        let placeholder = fixture.resource("filesystem", Id::now_v7());
        let scope = ResourceScope(&placeholder);
        let resource = fixture.filesystems[0]
            .create_resource(&scope, None)
            .await
            .expect("filesystem catalog create");
        let selection = fixture.resource("filesystem", resource.id.as_uuid());
        fixture
            .selected
            .lock()
            .expect("selection")
            .push(selection.clone());
        fixture.filesystems[1]
            .register_selected_resource(&scope, resource.id)
            .await
            .expect("right selected");
        let left = fixture.filesystems[0]
            .open_resource(&scope, resource.id)
            .await
            .expect("left filesystem");
        let right = fixture.filesystems[1]
            .open_resource(&scope, resource.id)
            .await
            .expect("right filesystem");
        left.write("initial.txt", b"initial")
            .await
            .expect("local content");
        pairs.push((left, right));
    }
    fixture.peers(true);
    fixture.start();
    for (_, right) in &pairs {
        until(|| async { right.entry("initial.txt").await.is_ok() }).await;
    }
    for (left, right) in &pairs {
        left.write("later.txt", b"later")
            .await
            .expect("write during persistent sessions");
        until(|| async { right.entry("later.txt").await.is_ok() }).await;
    }
    fixture.allowed.store(false, Ordering::SeqCst);
    fixture.policy_changed().await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    for (left, right) in &pairs {
        left.write("denied.txt", b"denied")
            .await
            .expect("local content remains writable");
        assert!(right.entry("denied.txt").await.is_err());
    }
    fixture.allowed.store(true, Ordering::SeqCst);
    fixture.policy_changed().await;
    for (_, right) in &pairs {
        until(|| async { right.entry("denied.txt").await.is_ok() }).await;
    }
    let resource = fixture.selected.lock().expect("selection")[0].clone();
    let scope = ResourceScope(&resource);
    let id = file_system::FileSystemId::parse(&resource.resource_id).expect("filesystem ID");
    fixture.filesystems[1]
        .delete_resource(&scope, id)
        .await
        .expect("higher-ID local deletion");
    until(|| async {
        fixture.filesystems[0]
            .is_tombstoned(&scope, id)
            .await
            .expect("lower-ID catalog")
    })
    .await;
    fixture.close().await;
}

#[tokio::test]
async fn closed_policy_feed_cancels_workers() {
    let mut fixture = Fixture::new().await;
    let (_, _, kv) = fixture.database().await;
    fixture.peers(true);
    fixture.start();
    kv[1]
        .client()
        .set("before", ofdb_kv::Value::Integer(1), None)
        .await
        .expect("KV write");
    until(|| async {
        kv[0]
            .client()
            .get("before")
            .await
            .expect("KV read")
            .is_some()
    })
    .await;
    fixture
        .policy
        .retire_and_remove()
        .await
        .expect("close authority backend");
    until(|| async {
        fixture.runtimes.iter().all(|runtime| {
            runtime
                .sync_task
                .as_ref()
                .is_some_and(tokio::task::JoinHandle::is_finished)
        })
    })
    .await;
    kv[1]
        .client()
        .set("after", ofdb_kv::Value::Integer(2), None)
        .await
        .expect("local write after feed closure");
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(
        kv[0]
            .client()
            .get("after")
            .await
            .expect("KV read")
            .is_none()
    );
    fixture.close().await;
}

#[tokio::test]
async fn never_opened_selections_can_be_cleaned_without_peers() {
    let fixture = Fixture::new().await;
    let selections = [
        fixture.resource("database", Id::now_v7()),
        fixture.resource("filesystem", Id::now_v7()),
    ];
    fixture
        .selected
        .lock()
        .expect("selection")
        .extend(selections.clone());
    assert!(
        fixture.runtimes[0]
            .host
            .configure()
            .await
            .expect("track selected resources without peers")
            .is_empty()
    );
    fixture.selected.lock().expect("selection").clear();
    assert!(
        fixture.runtimes[0]
            .host
            .configure()
            .await
            .expect("cleanup absent local copies completes")
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn cloned_peer_store_add_remove_reconfigures_without_host_notifications() {
    let mut fixture = Fixture::new().await;
    let (_, _, kv) = fixture.database().await;
    let peers = fixture
        .servers
        .iter()
        .map(|server| server.peers().clone())
        .collect::<Vec<_>>();
    fixture.start();
    until(|| async { fixture.calls.load(Ordering::SeqCst) >= 2 }).await;
    for side in 0..2 {
        assert!(peers[side].add(fixture.servers[1 - side].endpoint().id()));
    }
    kv[1]
        .client()
        .set("first", ofdb_kv::Value::Integer(1), None)
        .await
        .expect("local write");
    until(|| async {
        kv[0]
            .client()
            .get("first")
            .await
            .expect("read KV")
            .is_some()
    })
    .await;
    for side in 0..2 {
        assert!(peers[side].remove(fixture.servers[1 - side].endpoint().id()));
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    kv[1]
        .client()
        .set("after_removal", ofdb_kv::Value::Integer(2), None)
        .await
        .expect("local write");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        kv[0]
            .client()
            .get("after_removal")
            .await
            .expect("read KV")
            .is_none()
    );
    for side in 0..2 {
        assert!(peers[side].add(fixture.servers[1 - side].endpoint().id()));
    }
    until(|| async {
        kv[0]
            .client()
            .get("after_removal")
            .await
            .expect("read KV")
            .is_some()
    })
    .await;
    fixture.close().await;
}

#[tokio::test]
async fn configuration_self_write_finishes_then_coalesces_without_read_commit_loops() {
    let mut fixture = Fixture::new().await;
    let gate = crate::execution_test_support::Gate::default();
    let completed = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let policy = fixture.policy.clone();
    let callback_gate = gate.clone();
    let callback_completed = completed.clone();
    let callback_calls = calls.clone();
    let owner = fixture.owner.clone();
    let device_id = Id::now_v7();
    fixture.runtimes[0].host.authorizer = LocalReplicationAuthorizer::new(move |endpoint| {
        let policy = policy.clone();
        let gate = callback_gate.clone();
        let completed = callback_completed.clone();
        let first = callback_calls.fetch_add(1, Ordering::SeqCst) == 0;
        let owner_subject = owner.clone();
        async move {
            if first {
                sql(
                    &policy,
                    &format!(
                        "INSERT INTO events VALUES (CAST('{}' AS UUID))",
                        Id::now_v7()
                    ),
                )
                .await;
                gate.pause().await;
                completed.store(true, Ordering::SeqCst);
            }
            policy
                .client()
                .translate_and_execute("SELECT id FROM events", &db::SqlTranslator)
                .await
                .map_err(|error| error.to_string())?;
            Ok(DeviceReplicationPolicy {
                device_id,
                endpoint_id: endpoint,
                owner_subject,
                state: DeviceState::Approved,
                owner_active: true,
                admin_allowed: true,
                resources: vec![],
            })
        }
    });
    fixture.runtimes[0]
        .start_sync(fixture.cancellation.clone())
        .expect("start with policy feed");
    tokio::time::timeout(Duration::from_secs(5), gate.reached())
        .await
        .expect("configuration made its cleanup write");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "self-write must not cancel and restart its in-flight callback"
    );
    gate.release();
    until(|| async { completed.load(Ordering::SeqCst) && calls.load(Ordering::SeqCst) >= 2 }).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "one self-write causes one follow-up pass; authority reads stay silent"
    );
    fixture.close().await;
}

async fn wait_for_kv(store: &ofdb_kv::Database, key: &str, expected: ofdb_kv::Value) {
    let mut changes = store.sync_changes().expect("observe replication commits");
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if store.client().get(key).await.expect("read replicated KV") == Some(expected.clone())
            {
                return;
            }
            changes.changed().await.expect("KV subscription stays open");
        }
    })
    .await
    .expect("KV arrives from the admitted worker");
}

#[tokio::test]
async fn policy_invalidation_after_catalog_wake_clears_finished_workers_before_readmission() {
    let mut fixture = Fixture::new().await;
    let (resource, _, kv) = fixture.database().await;
    let catalog_wake = crate::execution_test_support::Gate::default();
    fixture.runtimes[1].catalog_wake = Some(catalog_wake.clone());
    // The lower-ID outbound path remains unavailable, so only the higher-ID job can replicate.
    fixture.accepting[1].store(false, Ordering::SeqCst);
    fixture.peers(true);
    fixture.start();
    kv[1]
        .client()
        .set("before", ofdb_kv::Value::Integer(1), None)
        .await
        .expect("source commit");
    wait_for_kv(&kv[0], "before", ofdb_kv::Value::Integer(1)).await;
    let mut policy_notice = fixture.runtimes[1].changes.subscribe();
    let job_stopped = fixture.runtimes[1].host.job_stopped.clone();
    fixture.databases[1]
        .create(&ResourceScope(&resource), None)
        .expect("wake supervisor from its catalog select");
    tokio::time::timeout(Duration::from_secs(5), catalog_wake.reached())
        .await
        .expect("supervisor paused immediately after catalog wake");
    fixture.policy_changed().await;
    tokio::time::timeout(Duration::from_secs(5), policy_notice.changed())
        .await
        .expect("policy watcher runs independently of supervisor")
        .expect("policy feed open");
    tokio::time::timeout(Duration::from_secs(5), job_stopped.notified())
        .await
        .expect("old worker finished while supervisor is paused");
    assert!(!fixture.runtimes[1].host.database.admission.allows());
    kv[1]
        .client()
        .set("after", ofdb_kv::Value::Integer(2), None)
        .await
        .expect("write during invalidation");
    assert!(
        kv[0]
            .client()
            .get("after")
            .await
            .expect("target KV")
            .is_none()
    );
    catalog_wake.release();
    wait_for_kv(&kv[0], "after", ofdb_kv::Value::Integer(2)).await;
    assert!(
        !policy_notice.has_changed().expect("policy watcher alive"),
        "re-admission requires no second policy notification"
    );
    fixture.close().await;
}

#[tokio::test]
async fn finished_jobs_restart_unless_their_tombstone_was_delivered() {
    let fixture = Fixture::new().await;
    fixture.peers(true);
    let host = &fixture.runtimes[1].host;
    assert!(
        host.database
            .admission
            .open(host.database.admission.generation())
    );
    let peer = fixture.servers[0].endpoint().id();
    let mut desired = std::collections::BTreeMap::new();
    let mut jobs = super::Jobs::default();
    let mut keys = Vec::new();
    let mut task_ids = Vec::new();
    for (index, deleted) in [false, false, true, true].into_iter().enumerate() {
        let resource = fixture.resource("database", Id::now_v7());
        let key = (
            peer,
            resource.owner_subject.clone(),
            resource.application_id.clone(),
            resource.kind.clone(),
            resource.resource_id.clone(),
            deleted,
        );
        let task = tokio::spawn(async move {
            if index == 0 {
                std::future::pending::<()>().await;
            }
            index == 3
        });
        task_ids.push(task.id());
        jobs.0.insert(key.clone(), super::Job::Running(task));
        desired.insert(key.clone(), resource);
        keys.push(key);
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if keys[1..].iter().all(|key| matches!(jobs.0.get(key), Some(super::Job::Running(task)) if task.is_finished())) { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("controlled jobs complete before reconfiguration");
    host.configure_jobs(&mut jobs, desired.clone()).await;
    for (index, key) in keys[..3].iter().enumerate() {
        let Some(super::Job::Running(task)) = jobs.0.get(key) else {
            panic!("live or undelivered job needs a worker");
        };
        if index == 0 {
            assert_eq!(task.id(), task_ids[index], "healthy worker is retained");
        } else {
            assert_ne!(task.id(), task_ids[index], "finished worker is replaced");
        }
    }
    assert!(matches!(
        jobs.0.get(&keys[3]),
        Some(super::Job::TombstoneDelivered)
    ));
    host.configure_jobs(&mut jobs, desired).await;
    assert!(
        matches!(jobs.0.get(&keys[3]), Some(super::Job::TombstoneDelivered)),
        "successful tombstone delivery must not be restarted"
    );
    jobs.cancel().await;
    fixture.close().await;
}

fn on_two_mib_workers(work: impl Future<Output = ()>) {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(2 * 1024 * 1024)
        .enable_all()
        .build()
        .expect("runtime with production-sized worker stacks")
        .block_on(work);
}

#[test]
fn responder_only_peer_removal_cancels_persistent_filesystem_session() {
    on_two_mib_workers(async {
        let mut fixture = Fixture::new().await;
        let (resource, files) = fixture.filesystem().await;
        fixture.peers(true);
        fixture.start();
        files[0]
            .write("before.txt", b"before")
            .await
            .expect("local write");
        until(|| async { files[1].entry("before.txt").await.is_ok() }).await;
        assert!(
            Arc::strong_count(&files[1]) > 2,
            "responder holds its persistent filesystem session"
        );
        let connection = fixture.servers[1]
            .connect_direct_with_alpn(fixture.servers[0].endpoint().id(), DATA_ALPN)
            .await
            .expect("authenticated connection remains open");
        let descriptor = crate::storage_protocol::descriptor(&resource);
        assert!(
            fixture.runtimes[1]
                .host
                .file_system
                .authorize(&descriptor, &connection)
                .await
        );
        assert!(
            fixture.servers[1]
                .peers()
                .remove(fixture.servers[0].endpoint().id())
        );
        assert!(
            !fixture.runtimes[1]
                .host
                .file_system
                .authorize(&descriptor, &connection)
                .await,
            "current inventory fences callbacks even on an established connection"
        );
        until(|| async { Arc::strong_count(&files[1]) == 2 }).await;
        assert!(
            fixture.servers[0]
                .peers()
                .contains(fixture.servers[1].endpoint().id()),
            "initiator inventory is unchanged"
        );
        files[0]
            .write("after.txt", b"after")
            .await
            .expect("local write");
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(files[1].entry("after.txt").await.is_err());
        fixture.servers[1]
            .peers()
            .add(fixture.servers[0].endpoint().id());
        until(|| async { files[1].entry("after.txt").await.is_ok() }).await;
        fixture.close().await;
    });
}

#[test]
fn invalidation_and_feed_closure_fence_sessions_while_configuration_is_blocked() {
    on_two_mib_workers(async {
        for close_feed in [false, true] {
            let mut fixture = Fixture::new().await;
            let (resource, files) = fixture.filesystem().await;
            let gate = crate::execution_test_support::Gate::default();
            let armed = Arc::new(AtomicBool::new(false));
            let entered = Arc::new(AtomicUsize::new(0));
            let original = fixture.runtimes[0].host.authorizer.clone();
            let callback_gate = gate.clone();
            let callback_armed = armed.clone();
            let callback_entered = entered.clone();
            let owner = fixture.owner.clone();
            let local = fixture.servers[0].endpoint().id();
            let devices = [Id::now_v7(), Id::now_v7()];
            fixture.runtimes[0].host.authorizer =
                LocalReplicationAuthorizer::new(move |endpoint| {
                    let original = original.clone();
                    let gate = callback_gate.clone();
                    let armed = callback_armed.clone();
                    let entered = callback_entered.clone();
                    let owner = owner.clone();
                    async move {
                        if endpoint == local && armed.swap(false, Ordering::SeqCst) {
                            entered.fetch_add(1, Ordering::SeqCst);
                            gate.pause().await;
                        }
                        let resources = original.selected_resources(endpoint).await?;
                        Ok(DeviceReplicationPolicy {
                            device_id: devices[usize::from(endpoint != local)],
                            endpoint_id: endpoint,
                            owner_subject: owner,
                            state: DeviceState::Approved,
                            owner_active: true,
                            admin_allowed: true,
                            resources,
                        })
                    }
                });
            fixture.peers(true);
            fixture.start();
            files[0]
                .write("before.txt", b"before")
                .await
                .expect("write before blocking configure");
            until(|| async { files[1].entry("before.txt").await.is_ok() }).await;
            armed.store(true, Ordering::SeqCst);
            fixture.filesystems[0]
                .create_resource(&ResourceScope(&resource), None)
                .await
                .expect("catalog event starts a configuration pass");
            tokio::time::timeout(Duration::from_secs(5), gate.reached())
                .await
                .expect("configure stalled while old worker is live");
            assert!(Arc::strong_count(&files[0]) > 2);
            if close_feed {
                fixture
                    .policy
                    .retire_and_remove()
                    .await
                    .expect("close authority feed while configure is blocked");
            } else {
                fixture.allowed.store(false, Ordering::SeqCst);
                fixture.policy_changed().await;
            }
            until(|| async {
                Arc::strong_count(&files[0]) == 2 && Arc::strong_count(&files[1]) == 2
            })
            .await;
            assert!(
                !fixture.runtimes[0].host.database.admission.allows(),
                "blocked configure cannot keep or reopen admission"
            );
            assert_eq!(
                entered.load(Ordering::SeqCst),
                1,
                "invalidation must not restart its blocked cleanup"
            );
            if close_feed {
                until(|| async {
                    fixture.runtimes[0]
                        .sync_task
                        .as_ref()
                        .expect("supervisor")
                        .is_finished()
                })
                .await;
            } else {
                assert!(
                    !fixture.runtimes[0]
                        .sync_task
                        .as_ref()
                        .expect("supervisor")
                        .is_finished(),
                    "cleanup is still blocked, but workers are already fenced"
                );
            }
            files[0]
                .write("after.txt", b"after")
                .await
                .expect("local data remains writable");
            assert!(files[1].entry("after.txt").await.is_err());
            gate.release();
            fixture.close().await;
        }
    });
}

#[test]
fn cancellation_and_drop_stop_owned_tasks_with_authorities_alive() {
    on_two_mib_workers(async {
        for drop_runtime in [false, true] {
            let mut fixture = Fixture::new().await;
            fixture.start();
            let runtime = fixture.runtimes.pop().expect("owned runtime");
            let policy_task = runtime
                .policy_task
                .as_ref()
                .expect("policy watcher")
                .abort_handle();
            let sync_task = runtime
                .sync_task
                .as_ref()
                .expect("supervisor")
                .abort_handle();
            let admission = runtime.host.database.admission.clone();
            if drop_runtime {
                drop(runtime);
            } else {
                fixture.cancellation.cancel();
                until(|| async { policy_task.is_finished() && sync_task.is_finished() }).await;
                runtime.shutdown().await.expect("join cancelled tasks");
            }
            until(|| async { policy_task.is_finished() && sync_task.is_finished() }).await;
            assert!(!admission.allows());
            fixture
                .policy
                .client()
                .translate_and_execute("SELECT id FROM events", &db::SqlTranslator)
                .await
                .expect("authority still alive after runtime stops");
            fixture.close().await;
        }
    });
}

#[tokio::test]
async fn policy_feed_is_required_before_background_start() {
    let mut fixture = Fixture::new().await;
    let runtime = &mut fixture.runtimes[0];
    runtime
        .policy_task
        .take()
        .expect("installed watcher")
        .abort();
    assert_eq!(
        runtime
            .start_sync(fixture.cancellation.clone())
            .expect_err("no silent replication without invalidations")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    fixture.close().await;
}

struct AcceptSwitch {
    handler: crate::DataProtocolHandler,
    enabled: Arc<AtomicBool>,
    rejected: Arc<AtomicUsize>,
}

impl iroh::protocol::ProtocolHandler for AcceptSwitch {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        if !self.enabled.load(Ordering::SeqCst) {
            self.rejected.fetch_add(1, Ordering::SeqCst);
            connection.close(0u32.into(), b"test transport unavailable");
            return Ok(());
        }
        self.handler.accept(connection).await
    }
}

impl std::fmt::Debug for AcceptSwitch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcceptSwitch").finish_non_exhaustive()
    }
}
