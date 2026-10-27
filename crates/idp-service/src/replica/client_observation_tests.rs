use std::{
    ops::Bound,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use chrono::Utc;
use futures::Stream;
use idp_model::{
    contract::{ClientProfile, ClientType, GrantType, TokenEndpointAuthMethod},
    model::{Client, Id},
};
use ofdb_sql::{
    AutomergeRowCodec, Engine, EngineExecutor, EngineResult, Kernel, KernelTransaction, RedbKernel,
    RowCodec, Server, redb,
};
use tokio::{sync::oneshot, task::JoinHandle};

use crate::{
    replica::DbKeyRepo,
    repo::{ClientRepo, KeyService},
};

use super::{
    ClientRow, DbClientRepo, Query, QueryInsert, Row, Statement, Value, local_credential_insert,
    tests::TestPrivateKeyRepo,
};

#[derive(Default)]
struct CatalogReads {
    tables: AtomicUsize,
    fields: AtomicUsize,
}

impl CatalogReads {
    fn reset(&self) {
        self.tables.store(0, Ordering::Relaxed);
        self.fields.store(0, Ordering::Relaxed);
    }

    fn totals(&self) -> [usize; 2] {
        [
            self.tables.load(Ordering::Relaxed),
            self.fields.load(Ordering::Relaxed),
        ]
    }
}

#[derive(Clone)]
struct CountingKernel {
    inner: RedbKernel,
    reads: Arc<CatalogReads>,
}

struct CountingTransaction {
    inner: <RedbKernel as Kernel>::Transaction,
    reads: Arc<CatalogReads>,
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

struct Fixture {
    directory: PathBuf,
    engine: Engine<CountingKernel, AutomergeRowCodec>,
    kernel: CountingKernel,
    repo: DbClientRepo<TestPrivateKeyRepo>,
    reads: Arc<CatalogReads>,
    client: Client,
    stop: Option<oneshot::Sender<()>>,
    server: JoinHandle<()>,
}

impl Fixture {
    async fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("client-observation-{}", Id::now_v7()));
        std::fs::create_dir(&directory).expect("isolated observation directory");
        let reads = Arc::new(CatalogReads::default());
        let kernel = CountingKernel {
            inner: RedbKernel::new(Arc::new(
                redb::Database::create(directory.join("identity.redb"))
                    .expect("native observation database"),
            )),
            reads: Arc::clone(&reads),
        };
        let engine = Engine::new(kernel.clone(), AutomergeRowCodec::new());
        let migration =
            include_str!("../../../idp-model/replica_migrations/20241117224814_init.up.sql");
        for statement in migration.split(';').map(str::trim).filter(|statement| {
            statement.starts_with("CREATE TABLE IF NOT EXISTS clients (")
                || statement.starts_with("CREATE TABLE IF NOT EXISTS local_client_credentials (")
        }) {
            engine
                .translate_and_execute(statement, &db::SqlTranslator)
                .await
                .expect("actual owner client schemas");
        }
        let now = Utc::now();
        let client = Client {
            id: Id::now_v7(),
            application_id: Id::now_v7(),
            client_id: "observation-client".into(),
            client_secret_hash: "first-local-verifier".into(),
            client_id_issued_at: None,
            client_secret_expires_at: None,
            client_name: "Observation".into(),
            client_uri: "https://app.example".into(),
            redirect_uris: vec![],
            client_type: ClientType::Confidential,
            profile: ClientProfile::Web,
            token_endpoint_auth_method: TokenEndpointAuthMethod::ClientSecretPost,
            allowed_grant_types: vec![GrantType::ClientCredentials],
            response_types: vec![],
            allowed_scopes: vec![],
            allowed_audiences: vec![],
            logo_uri: None,
            contacts: vec![],
            terms_of_service_uri: None,
            policy_uri: None,
            software_statement: None,
            software_id: None,
            software_version: None,
            created_at: now,
            updated_at: now,
        };
        let revision = Id::now_v7();
        let mut metadata = ClientRow::from(&client);
        metadata.credential_id = Some(revision);
        engine
            .execute(vec![
                Statement::Query(Query::Insert(QueryInsert {
                    table: "clients".into(),
                    row: metadata.into(),
                    returning: None,
                })),
                Statement::Query(local_credential_insert(&client, revision)),
            ])
            .await
            .expect("native client and matching credential fixture");
        let socket = directory.join("sql.sock");
        let (stop, stopped) = oneshot::channel();
        let serving = Server::unix(&socket, EngineExecutor::new(engine.clone()));
        let server = tokio::spawn(async move {
            serving
                .serve(async {
                    let _ = stopped.await;
                })
                .await
                .expect("SQL server");
        });
        let remote = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(remote) = ofdb_sql::Client::connect_unix(&socket).await {
                    break remote;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("bounded local SQL listener startup")
        .with_deadline(Duration::from_secs(5));
        let keys = Arc::new(KeyService::new(
            DbKeyRepo::new(remote.clone()),
            TestPrivateKeyRepo::default(),
            "client-observation",
        ));
        reads.reset();
        Self {
            directory,
            engine,
            kernel,
            repo: DbClientRepo::new(remote, keys),
            reads,
            client,
            stop: Some(stop),
            server,
        }
    }

    async fn sql(&self, sql: &str) {
        tokio::time::timeout(
            Duration::from_secs(5),
            self.engine.translate_and_execute(sql, &db::SqlTranslator),
        )
        .await
        .expect("native admission released after lookup")
        .expect("fixture SQL mutation");
    }

    async fn insert(&self, client: &Client) {
        let revision = Id::now_v7();
        let mut metadata = ClientRow::from(client);
        metadata.credential_id = Some(revision);
        self.engine
            .execute(vec![
                Statement::Query(Query::Insert(QueryInsert {
                    table: "clients".into(),
                    row: metadata.into(),
                    returning: None,
                })),
                Statement::Query(local_credential_insert(client, revision)),
            ])
            .await
            .expect("additional client fixture");
    }

    async fn conflict(&self, table: &str, id: Id, column: usize) {
        let left = AutomergeRowCodec::new();
        let right = AutomergeRowCodec::new();
        let mut transaction = self.kernel.transaction().await.expect("conflict fixture");
        let row = left
            .get_row(&transaction, table, &id)
            .await
            .expect("read fixture row")
            .expect("row exists");
        let mut first = row.clone();
        first.values[column] = Value::from("left");
        let mut second = row;
        second.values[column] = Value::from("right");
        let first = left
            .encode_row(&transaction, table, &id, &first, &[column])
            .await
            .expect("first concurrent change");
        let second = right
            .encode_row(&transaction, table, &id, &second, &[column])
            .await
            .expect("second concurrent change");
        for change in [first, second] {
            left.merge_row(&mut transaction, table, id, &change)
                .await
                .expect("merge concurrent evidence");
        }
        transaction.commit().await.expect("commit conflict fixture");
    }

    async fn finish(mut self) {
        let _ = self.stop.take().expect("server shutdown sender").send(());
        (&mut self.server).await.expect("SQL server shutdown");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        std::fs::remove_dir_all(&self.directory).expect("remove owned observation fixture");
    }
}

#[tokio::test]
async fn singleton_lookup_reuses_one_native_catalog() {
    let fixture = Fixture::new().await;
    let client = fixture
        .repo
        .find_client_by_client_id(&fixture.client.client_id)
        .await
        .expect("current client observation")
        .expect("live client");
    assert_eq!(client.client_secret_hash, "first-local-verifier");
    let scans = fixture.reads.totals();
    fixture.finish().await;
    assert_eq!(scans, [1, 1], "one complete table/field catalog per lookup");
}

#[tokio::test]
async fn by_id_and_metadata_lookups_each_start_a_fresh_catalog() {
    let fixture = Fixture::new().await;
    for _ in 0..2 {
        fixture.reads.reset();
        let client = fixture
            .repo
            .find_client_by_id(fixture.client.id)
            .await
            .expect("fresh by-ID observation")
            .expect("live client");
        assert_eq!(client.client_secret_hash, "first-local-verifier");
        assert_eq!(fixture.reads.totals(), [1, 1]);
    }
    fixture.reads.reset();
    let metadata = fixture
        .repo
        .find_client_registration(&fixture.client.client_id)
        .await
        .expect("fresh metadata observation")
        .expect("live registration");
    assert!(metadata.client_secret_hash.is_empty());
    assert_eq!(fixture.reads.totals(), [1, 1]);
    fixture.finish().await;
}

#[tokio::test]
async fn each_lookup_observes_current_matching_missing_and_rotated_credentials() {
    let fixture = Fixture::new().await;
    let first = fixture
        .repo
        .find_client_by_id(fixture.client.id)
        .await
        .expect("initial observation")
        .expect("client");
    assert_eq!(first.client_secret_hash, "first-local-verifier");
    let revision = Id::now_v7();
    fixture
        .sql(&format!(
            "UPDATE clients SET credential_id = CAST('{revision}' AS UUID)"
        ))
        .await;
    let mismatched = fixture
        .repo
        .find_client_by_client_id(&fixture.client.client_id)
        .await
        .expect("learned public revision")
        .expect("metadata remains");
    assert!(mismatched.client_secret_hash.is_empty());
    fixture
        .sql(&format!(
            "UPDATE local_client_credentials SET credential_id = CAST('{revision}' AS UUID), secret_hash = 'new-local-verifier'"
        ))
        .await;
    let matching = fixture
        .repo
        .find_client_by_id(fixture.client.id)
        .await
        .expect("new matching local revision")
        .expect("client");
    assert_eq!(matching.client_secret_hash, "new-local-verifier");
    fixture.sql("DELETE FROM local_client_credentials").await;
    let missing = fixture
        .repo
        .find_client_by_id(fixture.client.id)
        .await
        .expect("missing credential preserves metadata")
        .expect("client");
    assert!(missing.client_secret_hash.is_empty());
    fixture.finish().await;
}

#[tokio::test]
async fn metadata_lookup_needs_no_local_credential_storage() {
    let fixture = Fixture::new().await;
    fixture.sql("DROP TABLE local_client_credentials").await;
    let metadata = fixture
        .repo
        .find_client_registration(&fixture.client.client_id)
        .await
        .expect("metadata-only observation")
        .expect("registration remains");
    assert!(metadata.client_secret_hash.is_empty());
    assert!(
        fixture
            .repo
            .find_client_by_id(fixture.client.id)
            .await
            .is_err()
    );
    fixture
        .sql("UPDATE clients SET client_name = 'still writable'")
        .await;
    fixture.finish().await;
}

#[tokio::test]
async fn next_lookup_observes_client_revocation() {
    let fixture = Fixture::new().await;
    assert!(
        fixture
            .repo
            .find_client_by_id(fixture.client.id)
            .await
            .expect("initial observation")
            .is_some()
    );
    fixture.sql("UPDATE clients SET revoked_at = 1").await;
    assert!(
        fixture
            .repo
            .find_client_by_client_id(&fixture.client.client_id)
            .await
            .expect("revocation observed")
            .is_none()
    );
    assert!(
        fixture
            .repo
            .find_client_registration(&fixture.client.client_id)
            .await
            .expect("metadata observes revocation")
            .is_none()
    );
    fixture.finish().await;
}

#[tokio::test]
async fn both_conflict_checks_deny_and_next_lookup_observes_resolution() {
    for (table, column, name) in [
        ("local_client_credentials", 2, "secret_hash"),
        ("clients", 6, "client_name"),
    ] {
        let fixture = Fixture::new().await;
        assert!(
            fixture
                .repo
                .find_client_by_id(fixture.client.id)
                .await
                .expect("initial clear observation")
                .is_some()
        );
        fixture.conflict(table, fixture.client.id, column).await;
        assert!(
            fixture
                .repo
                .find_client_by_id(fixture.client.id)
                .await
                .is_err(),
            "{table} conflict must deny the by-ID lookup"
        );
        assert!(
            fixture
                .repo
                .find_client_by_client_id(&fixture.client.client_id)
                .await
                .is_err(),
            "{table} conflict must deny the external-ID lookup"
        );
        let metadata = fixture
            .repo
            .find_client_registration(&fixture.client.client_id)
            .await;
        if table == "clients" {
            assert!(metadata.is_err());
        } else {
            assert!(
                metadata
                    .expect("metadata does not hydrate credentials")
                    .expect("client")
                    .client_secret_hash
                    .is_empty()
            );
        }
        fixture
            .engine
            .resolve_row(
                table,
                &Row::new(vec![Value::Uuid(fixture.client.id)]),
                vec![(name.into(), Value::from("resolved"))],
            )
            .await
            .expect("resolve selected conflict after failed lookup released admission");
        let resolved = fixture
            .repo
            .find_client_by_id(fixture.client.id)
            .await
            .expect("next observation learns resolution")
            .expect("client remains");
        assert_eq!(
            resolved.client_secret_hash,
            if table == "clients" {
                "first-local-verifier"
            } else {
                "resolved"
            }
        );
        fixture.finish().await;
    }
}

#[tokio::test]
async fn pagination_hydrates_before_skipping_but_clears_only_selected_metadata() {
    let fixture = Fixture::new().await;
    let mut second = fixture.client.clone();
    second.id = Id::now_v7();
    second.client_id = "second-client".into();
    fixture.insert(&second).await;
    let listed = fixture.repo.list_clients(0, 2).await.expect("initial page");
    assert_eq!(listed.len(), 2);
    let skipped = listed[0].id;
    let selected = listed[1].id;
    fixture.conflict("clients", skipped, 6).await;
    let page = fixture
        .repo
        .list_clients(1, 1)
        .await
        .expect("excluded metadata is not cleared");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].id, selected);
    assert_eq!(page[0].client_secret_hash, "first-local-verifier");
    assert!(fixture.repo.list_clients(0, 1).await.is_err());
    fixture
        .conflict("local_client_credentials", skipped, 2)
        .await;
    assert!(
        fixture.repo.list_clients(1, 1).await.is_err(),
        "excluded credentials are still hydrated and conflict-checked before pagination"
    );
    fixture.finish().await;
}

#[tokio::test]
async fn cancelled_admission_does_not_prevent_a_fresh_lookup() {
    let fixture = Fixture::new().await;
    let held = fixture
        .kernel
        .transaction()
        .await
        .expect("hold actual native admission");
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            fixture.repo.find_client_by_id(fixture.client.id),
        )
        .await
        .is_err()
    );
    held.rollback()
        .await
        .expect("release held native admission");
    let client = fixture
        .repo
        .find_client_by_id(fixture.client.id)
        .await
        .expect("fresh lookup after cancelled admission")
        .expect("live client");
    assert_eq!(client.client_secret_hash, "first-local-verifier");
    fixture.finish().await;
}
