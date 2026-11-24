use std::{path::PathBuf, sync::Arc, time::Duration};

use chrono::Utc;
use of_model::id::Id;
use of_model::idp::contract::entity_type::EntityType;
use of_model::idp::contract::jwk_public::JwkPublic;
use of_model::idp::contract::jwk_public_parameters::JwkPublicParameters;
use of_model::idp::contract::jws_algorithm::JwsAlgorithm;
use of_model::idp::contract::key_use::KeyUse;
use ofdb_sql::{
    AutomergeRowCodec, Engine, EngineExecutor, Kernel, KernelTransaction, RedbKernel, RowCodec,
    Server, redb,
};
use tokio::{sync::oneshot, task::JoinHandle};

use crate::repo::KeyRepo;

use crate::ofdb::key_repo::DbKeyRepo;

use super::observation_test_kernel::CatalogReads;

use super::observation_test_kernel::CountingKernel;

struct Fixture {
    directory: PathBuf,
    engine: Engine<CountingKernel, AutomergeRowCodec>,
    kernel: CountingKernel,
    remote: ofdb_sql::Client,
    reads: Arc<CatalogReads>,
    stop: Option<oneshot::Sender<()>>,
    server: JoinHandle<()>,
}

impl Fixture {
    async fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("authority-observation-{}", Id::now_v7()));
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
        let migration = of_model::ofdb::MIGRATIONS
            .get_file("0001_init.up.sql")
            .and_then(|file| file.contents_utf8())
            .expect("embedded initial schema");
        for statement in migration
            .split(';')
            .map(str::trim)
            .filter(|statement| statement.starts_with("CREATE TABLE IF NOT EXISTS keys ("))
        {
            engine
                .translate_and_execute(statement, &ofdb_sql::SqlTranslator)
                .await
                .expect("actual owner authority schemas");
        }
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
        reads.reset();
        Self {
            directory,
            engine,
            kernel,
            remote,
            reads,
            stop: Some(stop),
            server,
        }
    }

    async fn sql(&self, sql: &str) {
        tokio::time::timeout(
            Duration::from_secs(5),
            self.engine
                .translate_and_execute(sql, &ofdb_sql::SqlTranslator),
        )
        .await
        .expect("native admission released after lookup")
        .expect("fixture SQL mutation");
    }

    async fn key(&self, entity: Id, parent: Option<Id>, created: i64) -> Id {
        let parent = parent.map_or("NULL".into(), |id| format!("CAST('{id}' AS UUID)"));
        let results = self.engine.translate_and_execute(&format!("INSERT INTO keys (parent_id, entity_type, entity_id, derivation_path, derivation_index, hardened, name, revoked_at, expires_at, created_at, updated_at, public_jwk) VALUES ({parent}, 0, CAST('{entity}' AS UUID), 'm/0', 0, 1, 'fixture', NULL, NULL, {created}, {created}, NULL) RETURNING id"), &ofdb_sql::SqlTranslator).await.expect("create fixture key");
        super::insert::generated_id(&results).expect("fixture key identity")
    }

    async fn conflict(&self, table: &str, id: Id, column: usize, values: [ofdb_sql::Value; 2]) {
        let mut transaction = self
            .kernel
            .transaction()
            .await
            .expect("conflict fixture transaction");
        let left = AutomergeRowCodec::new();
        let right = AutomergeRowCodec::new();
        let row = left
            .get_row(&transaction, table, &id)
            .await
            .expect("read fixture row")
            .expect("fixture row exists");
        let mut first = row.clone();
        first.values[column] = values[0].clone();
        let mut second = row;
        second.values[column] = values[1].clone();
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
        transaction.commit().await.expect("commit real conflict");
    }

    async fn replace_value(&self, table: &str, id: Id, column: usize, value: ofdb_sql::Value) {
        let mut transaction = self
            .kernel
            .transaction()
            .await
            .expect("malformed evidence fixture");
        let codec = AutomergeRowCodec::new();
        let mut row = codec
            .get_row(&transaction, table, &id)
            .await
            .expect("fixture row read")
            .expect("fixture row exists");
        row.values[column] = value;
        let change = codec
            .encode_row(&transaction, table, &id, &row, &[column])
            .await
            .expect("encode learned row evidence");
        codec
            .merge_row(&mut transaction, table, id, &change)
            .await
            .expect("merge learned row evidence");
        transaction
            .commit()
            .await
            .expect("commit learned row evidence");
    }

    async fn resolve(&self, table: &str, id: Id, column: &str, value: ofdb_sql::Value) {
        tokio::time::timeout(
            Duration::from_secs(5),
            self.engine.resolve_row(
                table,
                &ofdb_sql::Row::new(vec![ofdb_sql::Value::Uuid(id)]),
                vec![(column.into(), value)],
            ),
        )
        .await
        .expect("native admission released after refusal")
        .expect("explicit conflict resolution");
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
async fn key_read_preserves_every_decoded_field() {
    let fixture = Fixture::new().await;
    let parent = Id::now_v7();
    let entity = Id::now_v7();
    let id = fixture.key(entity, Some(parent), 10).await;
    let public: of_model::idp::contract::jwk_public::JwkPublic = serde_json::from_value(serde_json::json!({
        "kid": id.to_string(), "use": "sig", "alg": "ES256K", "kty": "Ec", "crv": "secp256k1",
        "x": "eb5mfvncu6xVoGKVzocLBwKb/NstzijZWfKBWxb4F5g", "y": "SDradyajxGVdpPv8DhEIqP0XtEimhVQZnEfQj/sQ1Lg",
    })).expect("public key fixture");
    let encoded = serde_json::to_string(&public).expect("public JWK JSON");
    let future = Utc::now().timestamp() + 3600;
    fixture.sql(&format!("UPDATE keys SET derivation_path = 'm/7/42', derivation_index = 42, hardened = 0, name = 'Decoded key', public_jwk = '{encoded}', revoked_at = {future}, expires_at = {future}, updated_at = 20 WHERE id = CAST('{id}' AS UUID)")).await;
    let expected = of_model::idp::model::key::Key {
        id,
        parent_id: Some(parent),
        entity_type: EntityType::User,
        entity_id: entity,
        derivation_path: "m/7/42".into(),
        derivation_index: 42,
        hardened: false,
        name: "Decoded key".into(),
        public_jwk: Some(public),
        revoked_at: chrono::DateTime::from_timestamp(future, 0),
        expires_at: chrono::DateTime::from_timestamp(future, 0),
        created_at: chrono::DateTime::from_timestamp(10, 0).expect("valid creation timestamp"),
        updated_at: chrono::DateTime::from_timestamp(20, 0).expect("valid update timestamp"),
    };
    assert_eq!(
        DbKeyRepo::new(fixture.remote.clone())
            .find_by_id(id)
            .await
            .expect("complete key read"),
        Some(expected)
    );
    fixture.finish().await;
}

#[tokio::test]
async fn singleton_key_read_reuses_one_native_catalog() {
    let fixture = Fixture::new().await;
    let id = fixture
        .key(Id::now_v7(), None, Utc::now().timestamp())
        .await;
    fixture.reads.reset();
    let key = DbKeyRepo::new(fixture.remote.clone())
        .find_by_id(id)
        .await
        .expect("key observation")
        .expect("live key");
    assert_eq!(key.id, id);
    let scans = fixture.reads.totals();
    fixture.finish().await;
    assert_eq!(
        scans,
        [1, 1],
        "one complete table/field catalog per singleton key read"
    );
}

#[tokio::test]
async fn key_read_methods_each_observe_a_fresh_catalog_and_rotation() {
    let fixture = Fixture::new().await;
    let repo = DbKeyRepo::new(fixture.remote.clone());
    let entity = Id::now_v7();
    let old = fixture.key(entity, None, 10).await;
    for expected in [old, Id::nil()] {
        let expected = if expected == old {
            old
        } else {
            fixture.key(entity, None, 20).await
        };
        fixture.reads.reset();
        assert_eq!(
            repo.find_by_id(expected)
                .await
                .expect("fresh ID read")
                .expect("active key")
                .id,
            expected
        );
        assert_eq!(fixture.reads.totals(), [1, 1]);
        fixture.reads.reset();
        let active = repo.list_active().await.expect("fresh active keys");
        assert!(active.iter().any(|key| key.id == expected));
        assert_eq!(active.len(), if expected == old { 1 } else { 2 });
        assert_eq!(fixture.reads.totals(), [1, 1]);
        fixture.reads.reset();
        assert_eq!(
            repo.list_by_entity_type_and_id(EntityType::User, entity)
                .await
                .expect("fresh entity keys"),
            active
        );
        assert_eq!(fixture.reads.totals(), [1, 1]);
        fixture.reads.reset();
        assert_eq!(
            repo.find_active_entity_root_key(EntityType::User, entity)
                .await
                .expect("latest root")
                .expect("root exists")
                .id,
            expected
        );
        assert_eq!(fixture.reads.totals(), [1, 1]);
        fixture.reads.reset();
        assert_eq!(
            repo.find_by_entity_type_and_id(EntityType::User, entity)
                .await
                .expect("root alias")
                .expect("root exists")
                .id,
            expected
        );
        assert_eq!(fixture.reads.totals(), [1, 1]);
    }
    fixture.finish().await;
}

#[tokio::test]
async fn key_reads_filter_current_revocation_expiry_and_entity_before_clearance() {
    let fixture = Fixture::new().await;
    let repo = DbKeyRepo::new(fixture.remote.clone());
    let entity = Id::now_v7();
    let active = fixture.key(entity, None, 10).await;
    let revoked = fixture.key(entity, None, 10).await;
    let expired = fixture.key(entity, None, 10).await;
    let other = fixture.key(Id::now_v7(), None, 10).await;
    let now = Utc::now().timestamp();
    fixture.sql(&format!("UPDATE keys SET revoked_at = {now} WHERE id = CAST('{revoked}' AS UUID); UPDATE keys SET expires_at = {now} WHERE id = CAST('{expired}' AS UUID); UPDATE keys SET revoked_at = {} , expires_at = {} WHERE id = CAST('{active}' AS UUID)", now + 3600, now + 3600)).await;
    for id in [revoked, expired, other] {
        fixture
            .conflict(
                "keys",
                id,
                7,
                [
                    ofdb_sql::Value::from("left"),
                    ofdb_sql::Value::from("right"),
                ],
            )
            .await;
    }
    assert!(
        repo.find_by_id(revoked)
            .await
            .expect("revoked conflict excluded")
            .is_none()
    );
    assert!(
        repo.find_by_id(expired)
            .await
            .expect("expired conflict excluded")
            .is_none()
    );
    assert_eq!(
        repo.list_by_entity_type_and_id(EntityType::User, entity)
            .await
            .expect("only active entity key cleared")
            .iter()
            .map(|key| key.id)
            .collect::<Vec<_>>(),
        vec![active]
    );
    assert!(
        repo.list_by_entity_type_and_id(EntityType::Client, entity)
            .await
            .expect("exact entity type")
            .is_empty()
    );
    assert!(
        repo.list_active().await.is_err(),
        "unrelated active key still belongs in the all-active read"
    );
    fixture
        .resolve("keys", other, "name", ofdb_sql::Value::from("resolved"))
        .await;
    assert_eq!(
        repo.list_active()
            .await
            .expect("inactive conflicts excluded")
            .len(),
        2
    );
    fixture
        .sql(&format!(
            "UPDATE keys SET revoked_at = {now} WHERE id = CAST('{active}' AS UUID)"
        ))
        .await;
    assert!(
        repo.find_by_id(active)
            .await
            .expect("next call sees revocation")
            .is_none()
    );
    assert!(
        repo.find_active_entity_root_key(EntityType::User, entity)
            .await
            .expect("no active root")
            .is_none()
    );
    fixture.finish().await;
}

#[tokio::test]
async fn older_root_and_child_conflicts_deny_before_latest_root_selection() {
    for conflict_child in [false, true] {
        let fixture = Fixture::new().await;
        let repo = DbKeyRepo::new(fixture.remote.clone());
        let entity = Id::now_v7();
        let old = fixture.key(entity, None, 10).await;
        let new = fixture.key(entity, None, 20).await;
        let child = fixture.key(entity, Some(new), 30).await;
        assert_eq!(
            repo.find_active_entity_root_key(EntityType::User, entity)
                .await
                .expect("newest root, not child")
                .expect("root")
                .id,
            new
        );
        let conflicted = if conflict_child { child } else { old };
        fixture
            .conflict(
                "keys",
                conflicted,
                7,
                [
                    ofdb_sql::Value::from("left"),
                    ofdb_sql::Value::from("right"),
                ],
            )
            .await;
        assert!(
            repo.find_active_entity_root_key(EntityType::User, entity)
                .await
                .is_err()
        );
        assert!(
            repo.find_by_entity_type_and_id(EntityType::User, entity)
                .await
                .is_err()
        );
        assert!(
            repo.list_by_entity_type_and_id(EntityType::User, entity)
                .await
                .is_err()
        );
        assert!(repo.list_active().await.is_err());
        assert!(repo.find_by_id(conflicted).await.is_err());
        assert_eq!(
            repo.find_by_id(new)
                .await
                .expect("unselected key conflict does not deny ID read")
                .expect("new root")
                .id,
            new
        );
        fixture
            .resolve(
                "keys",
                conflicted,
                "name",
                ofdb_sql::Value::from("resolved"),
            )
            .await;
        assert_eq!(
            repo.find_active_entity_root_key(EntityType::User, entity)
                .await
                .expect("fresh resolution")
                .expect("root")
                .id,
            new
        );
        fixture.finish().await;
    }
}

#[tokio::test]
async fn unrelated_malformed_key_rows_refuse_all_reads_and_release_admission() {
    let fixture = Fixture::new().await;
    let repo = DbKeyRepo::new(fixture.remote.clone());
    let entity = Id::now_v7();
    let good = fixture.key(entity, None, 10).await;
    let bad = fixture.key(Id::now_v7(), None, 10).await;
    for (bad_value, repair) in [
        ("public_jwk = 'malformed'", "public_jwk = NULL"),
        ("entity_type = 99", "entity_type = 0"),
        ("derivation_index = 4294967296", "derivation_index = 0"),
        ("created_at = 9223372036854775807", "created_at = 10"),
    ] {
        fixture
            .sql(&format!(
                "UPDATE keys SET {bad_value}, revoked_at = 1 WHERE id = CAST('{bad}' AS UUID)"
            ))
            .await;
        assert!(repo.find_by_id(good).await.is_err(), "{bad_value}");
        assert!(
            repo.find_by_id(Id::now_v7()).await.is_err(),
            "missing ID cannot skip decoding"
        );
        assert!(repo.list_active().await.is_err());
        assert!(
            repo.list_by_entity_type_and_id(EntityType::User, entity)
                .await
                .is_err()
        );
        assert!(
            repo.find_active_entity_root_key(EntityType::User, entity)
                .await
                .is_err()
        );
        fixture
            .sql(&format!(
                "UPDATE keys SET {repair} WHERE id = CAST('{bad}' AS UUID)"
            ))
            .await;
        assert_eq!(
            repo.find_by_id(good)
                .await
                .expect("next call sees repair")
                .expect("good key")
                .id,
            good
        );
    }
    fixture.finish().await;
}

#[tokio::test]
async fn empty_and_deleted_keys_are_fresh_on_the_next_call() {
    let fixture = Fixture::new().await;
    let repo = DbKeyRepo::new(fixture.remote.clone());
    let entity = Id::now_v7();
    let id = Id::now_v7();
    assert!(repo.list_active().await.expect("empty list").is_empty());
    assert!(repo.find_by_id(id).await.expect("missing key").is_none());
    assert!(
        repo.list_by_entity_type_and_id(EntityType::User, entity)
            .await
            .expect("empty entity")
            .is_empty()
    );
    let id = fixture.key(entity, None, 10).await;
    assert!(repo.find_by_id(id).await.expect("new key").is_some());
    repo.delete_by_entity_type_and_id(EntityType::User, entity)
        .await
        .expect("existing delete semantics");
    fixture.reads.reset();
    assert!(repo.find_by_id(id).await.expect("deleted key").is_none());
    assert_eq!(fixture.reads.totals(), [1, 1]);
    assert!(
        repo.find_active_entity_root_key(EntityType::User, entity)
            .await
            .expect("deleted root")
            .is_none()
    );
    fixture.finish().await;
}
