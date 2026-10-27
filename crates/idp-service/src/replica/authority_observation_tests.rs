use std::{path::PathBuf, sync::Arc, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use chrono::Utc;
use idp_model::{
    contract::{
        EntityType, IdpSignerRecord, JwkPublic, JwkPublicParameters, JwsAlgorithm, KeyUse,
        ReplicaMembership,
    },
    model::Id,
};
use k256::ecdsa::SigningKey;
use ofdb_sql::{
    AutomergeRowCodec, Engine, EngineExecutor, Kernel, KernelTransaction, RedbKernel, RowCodec,
    Server, redb,
};
use tokio::{sync::oneshot, task::JoinHandle};

use crate::repo::KeyRepo;

use super::{
    DbKeyRepo, DbReplicaSignerRepo,
    observation_test_kernel::{CatalogReads, CountingKernel},
};

struct Fixture {
    directory: PathBuf,
    engine: Engine<CountingKernel, AutomergeRowCodec>,
    kernel: CountingKernel,
    remote: db::Client,
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
        let migration =
            include_str!("../../../idp-model/replica_migrations/20241117224814_init.up.sql");
        for statement in migration.split(';').map(str::trim).filter(|statement| {
            statement.starts_with("CREATE TABLE IF NOT EXISTS keys (")
                || statement.starts_with("CREATE TABLE IF NOT EXISTS idp_replica_members (")
                || statement.starts_with("CREATE TABLE IF NOT EXISTS idp_replica_signers (")
        }) {
            engine
                .translate_and_execute(statement, &db::SqlTranslator)
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
            self.engine.translate_and_execute(sql, &db::SqlTranslator),
        )
        .await
        .expect("native admission released after lookup")
        .expect("fixture SQL mutation");
    }

    async fn key(&self, id: Id, entity: Id, parent: Option<Id>, created: i64) {
        let parent = parent.map_or("NULL".into(), |id| format!("CAST('{id}' AS UUID)"));
        self.sql(&format!("INSERT INTO keys (id, parent_id, entity_type, entity_id, derivation_path, derivation_index, hardened, name, revoked_at, expires_at, created_at, updated_at, public_jwk) VALUES (CAST('{id}' AS UUID), {parent}, 0, CAST('{entity}' AS UUID), 'm/0', 0, 1, 'fixture', NULL, NULL, {created}, {created}, NULL)")).await;
    }

    async fn conflict(&self, table: &str, id: Id, column: usize, values: [db::Value; 2]) {
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

    async fn replace_value(&self, table: &str, id: Id, column: usize, value: db::Value) {
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

    async fn resolve(&self, table: &str, id: Id, column: &str, value: db::Value) {
        tokio::time::timeout(
            Duration::from_secs(5),
            self.engine.resolve_row(
                table,
                &db::Row::new(vec![db::Value::Uuid(id)]),
                vec![(column.into(), value)],
            ),
        )
        .await
        .expect("native admission released after refusal")
        .expect("explicit conflict resolution");
    }

    async fn trust_row_id(&self, table: &str, member: Id) -> Id {
        let results = self
            .engine
            .translate_and_execute(
                &format!("SELECT id FROM {table} WHERE member_id = CAST('{member}' AS UUID)"),
                &db::SqlTranslator,
            )
            .await
            .expect("locate fixture row");
        db::decode(&results[0].rows[0].values[0], "id").expect("fixture row UUID")
    }

    async fn trust(&self) -> (ReplicaMembership, IdpSignerRecord) {
        let member_id = Id::now_v7();
        let key_id = Id::now_v7();
        let issuer = "https://installation.example".to_owned();
        let signing_key = SigningKey::from_slice(&[1_u8; 32]).expect("fixed valid scalar");
        let point = signing_key.verifying_key().to_encoded_point(false);
        let member = ReplicaMembership {
            installation_id: issuer.clone(),
            member_id,
            endpoint_id: member_id.to_string(),
            issuer: issuer.clone(),
            approved_at: 10,
            revoked_at: None,
        };
        let signer = IdpSignerRecord {
            member_id,
            key_id,
            issuer,
            public_jwk: JwkPublic {
                kid: key_id.to_string(),
                r#use: KeyUse::Signature,
                alg: JwsAlgorithm::ES256K,
                params: JwkPublicParameters::Ec {
                    crv: "secp256k1".into(),
                    x: STANDARD_NO_PAD.encode(point.x().expect("public x")),
                    y: STANDARD_NO_PAD.encode(point.y().expect("public y")),
                },
            },
            approved_at: Some(10),
            revoked_at: None,
            expires_at: None,
        };
        DbReplicaSignerRepo::new(self.remote.clone())
            .enroll(&member, &signer)
            .await
            .expect("public enrollment");
        (member, signer)
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
    let id = Id::now_v7();
    let parent = Id::now_v7();
    let entity = Id::now_v7();
    let (_, signer) = fixture.trust().await;
    let mut public = signer.public_jwk;
    public.kid = id.to_string();
    let encoded = serde_json::to_string(&public).expect("public JWK JSON");
    let future = Utc::now().timestamp() + 3600;
    fixture.key(id, entity, Some(parent), 10).await;
    fixture.sql(&format!("UPDATE keys SET derivation_path = 'm/7/42', derivation_index = 42, hardened = 0, name = 'Decoded key', public_jwk = '{encoded}', revoked_at = {future}, expires_at = {future}, updated_at = 20 WHERE id = CAST('{id}' AS UUID)")).await;
    let expected = idp_model::model::Key {
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
    let id = Id::now_v7();
    fixture
        .key(id, Id::now_v7(), None, Utc::now().timestamp())
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
async fn singleton_trust_read_reuses_one_native_catalog() {
    let fixture = Fixture::new().await;
    let pair = fixture.trust().await;
    fixture.reads.reset();
    let trusted = DbReplicaSignerRepo::new(fixture.remote.clone())
        .trusted_signers(&pair.0.issuer, 20)
        .await
        .expect("current trust observation");
    assert_eq!(trusted, vec![pair]);
    let scans = fixture.reads.totals();
    fixture.finish().await;
    assert_eq!(
        scans,
        [1, 1],
        "one complete table/field catalog for SELECTs and both conflict tables"
    );
}

#[tokio::test]
async fn key_read_methods_each_observe_a_fresh_catalog_and_rotation() {
    let fixture = Fixture::new().await;
    let repo = DbKeyRepo::new(fixture.remote.clone());
    let entity = Id::now_v7();
    let old = Id::now_v7();
    let new = Id::now_v7();
    fixture.key(old, entity, None, 10).await;
    for expected in [old, new] {
        if expected == new {
            fixture.key(new, entity, None, 20).await;
        }
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
    let active = Id::now_v7();
    let revoked = Id::now_v7();
    let expired = Id::now_v7();
    let other = Id::now_v7();
    for id in [active, revoked, expired] {
        fixture.key(id, entity, None, 10).await;
    }
    fixture.key(other, Id::now_v7(), None, 10).await;
    let now = Utc::now().timestamp();
    fixture.sql(&format!("UPDATE keys SET revoked_at = {now} WHERE id = CAST('{revoked}' AS UUID); UPDATE keys SET expires_at = {now} WHERE id = CAST('{expired}' AS UUID); UPDATE keys SET revoked_at = {} , expires_at = {} WHERE id = CAST('{active}' AS UUID)", now + 3600, now + 3600)).await;
    for id in [revoked, expired, other] {
        fixture
            .conflict(
                "keys",
                id,
                7,
                [db::Value::from("left"), db::Value::from("right")],
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
        .resolve("keys", other, "name", db::Value::from("resolved"))
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
        let old = Id::now_v7();
        let new = Id::now_v7();
        let child = Id::now_v7();
        fixture.key(old, entity, None, 10).await;
        fixture.key(new, entity, None, 20).await;
        fixture.key(child, entity, Some(new), 30).await;
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
                [db::Value::from("left"), db::Value::from("right")],
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
            .resolve("keys", conflicted, "name", db::Value::from("resolved"))
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
    let good = Id::now_v7();
    let bad = Id::now_v7();
    fixture.key(good, entity, None, 10).await;
    fixture.key(bad, Id::now_v7(), None, 10).await;
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
    fixture.key(id, entity, None, 10).await;
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

#[tokio::test]
async fn native_trust_reads_retain_exact_bindings_and_malformed_time_refusals() {
    use db::Value::{Integer, Null, Text, Uuid};
    for (table, column, value) in [
        (
            "idp_replica_members",
            1,
            Text("https://other.example".into()),
        ),
        ("idp_replica_members", 2, Uuid(Id::now_v7())),
        ("idp_replica_members", 3, Text(String::new())),
        ("idp_replica_members", 3, Text(" endpoint".into())),
        (
            "idp_replica_members",
            4,
            Text("https://other.example".into()),
        ),
        ("idp_replica_members", 5, Integer(-1)),
        ("idp_replica_members", 5, Integer(21)),
        ("idp_replica_members", 5, Null),
        ("idp_replica_members", 5, Text("malformed".into())),
        ("idp_replica_members", 6, Integer(21)),
        ("idp_replica_signers", 1, Uuid(Id::now_v7())),
        ("idp_replica_signers", 2, Null),
        ("idp_replica_signers", 2, Uuid(Id::now_v7())),
        ("idp_replica_signers", 3, Uuid(Id::now_v7())),
        (
            "idp_replica_signers",
            4,
            Text("https://other.example".into()),
        ),
        ("idp_replica_signers", 5, Text("malformed".into())),
        ("idp_replica_signers", 6, Integer(-1)),
        ("idp_replica_signers", 6, Integer(21)),
        ("idp_replica_signers", 6, Null),
        ("idp_replica_signers", 6, Text("malformed".into())),
        ("idp_replica_signers", 7, Integer(21)),
        ("idp_replica_signers", 8, Integer(-1)),
        ("idp_replica_signers", 8, Integer(20)),
        ("idp_replica_signers", 8, Text("malformed".into())),
    ] {
        let fixture = Fixture::new().await;
        let pair = fixture.trust().await;
        let id = fixture.trust_row_id(table, pair.0.member_id).await;
        fixture
            .replace_value(table, id, column, value.clone())
            .await;
        let result = DbReplicaSignerRepo::new(fixture.remote.clone())
            .trusted_signers(&pair.0.issuer, 20)
            .await;
        assert!(
            result.is_err() || result.expect("decoded trust").is_empty(),
            "accepted {table} column {column}: {value:?}"
        );
        fixture
            .sql("DELETE FROM idp_replica_signers; DELETE FROM idp_replica_members")
            .await;
        fixture.finish().await;
    }
}

#[tokio::test]
async fn trust_conflicts_preserve_pairs_inspect_both_tables_and_refresh() {
    let fixture = Fixture::new().await;
    let pairs = [
        fixture.trust().await,
        fixture.trust().await,
        fixture.trust().await,
    ];
    let repo = DbReplicaSignerRepo::new(fixture.remote.clone());
    let signer_row = fixture
        .trust_row_id("idp_replica_signers", pairs[0].0.member_id)
        .await;
    let member_row = fixture
        .trust_row_id("idp_replica_members", pairs[2].0.member_id)
        .await;
    fixture
        .conflict(
            "idp_replica_signers",
            signer_row,
            6,
            [db::Value::Integer(11), db::Value::Integer(12)],
        )
        .await;
    fixture
        .conflict(
            "idp_replica_members",
            member_row,
            3,
            [db::Value::from("left"), db::Value::from("right")],
        )
        .await;
    fixture.reads.reset();
    assert_eq!(
        repo.trusted_signers(&pairs[0].0.issuer, 20)
            .await
            .expect("independent conflict decisions"),
        vec![pairs[1].clone()]
    );
    assert_eq!(fixture.reads.totals(), [1, 1]);
    assert_eq!(
        fixture.reads.trust_documents(),
        [3, 3],
        "all selected signers and memberships inspected despite conflicts"
    );
    fixture
        .resolve(
            "idp_replica_signers",
            signer_row,
            "approved_at",
            db::Value::Integer(10),
        )
        .await;
    fixture
        .resolve(
            "idp_replica_members",
            member_row,
            "endpoint_id",
            db::Value::from(pairs[2].0.endpoint_id.as_str()),
        )
        .await;
    fixture.reads.reset();
    let trusted = repo
        .trusted_signers(&pairs[0].0.issuer, 20)
        .await
        .expect("fresh conflict resolution");
    assert_eq!(trusted.len(), 3);
    for pair in &pairs {
        assert!(trusted.contains(pair));
    }
    assert_eq!(fixture.reads.totals(), [1, 1]);
    repo.revoke(pairs[0].0.member_id, &pairs[0].0.issuer, 21)
        .await
        .expect("public revoke");
    assert_eq!(
        repo.trusted_signers(&pairs[0].0.issuer, 21)
            .await
            .expect("next call sees revocation")
            .len(),
        2
    );
    fixture.finish().await;
}

#[tokio::test]
async fn trust_reads_observe_empty_deleted_rotated_expired_and_malformed_state() {
    let fixture = Fixture::new().await;
    let repo = DbReplicaSignerRepo::new(fixture.remote.clone());
    assert!(
        repo.trusted_signers("https://installation.example", 20)
            .await
            .expect("empty trust")
            .is_empty()
    );
    let (member, mut signer) = fixture.trust().await;
    assert_eq!(
        repo.trusted_signers(&member.issuer, 20)
            .await
            .expect("new enrollment"),
        vec![(member.clone(), signer.clone())]
    );
    let old_key = signer.key_id;
    signer.key_id = Id::now_v7();
    signer.public_jwk.kid = signer.key_id.to_string();
    signer.approved_at = Some(21);
    signer.expires_at = Some(30);
    repo.rotate(member.member_id, &member.issuer, &signer)
        .await
        .expect("existing public rotation");
    assert_ne!(old_key, signer.key_id);
    fixture.reads.reset();
    assert_eq!(
        DbKeyRepo::new(fixture.remote.clone())
            .trusted_node_signers(&member.issuer, 21)
            .await
            .expect("fresh KeyRepo trust delegation"),
        vec![(member.clone(), signer.clone())]
    );
    assert_eq!(fixture.reads.totals(), [1, 1]);
    assert!(
        repo.trusted_signers(&member.issuer, 30)
            .await
            .expect("expiry boundary")
            .is_empty()
    );
    assert!(
        repo.trusted_signers("https://installation.example/", 21)
            .await
            .expect("exact issuer")
            .is_empty()
    );
    fixture
        .sql("UPDATE idp_replica_signers SET public_jwk = 'malformed' WHERE revoked_at IS NULL")
        .await;
    assert!(repo.trusted_signers(&member.issuer, 21).await.is_err());
    fixture.sql("DELETE FROM idp_replica_signers").await;
    fixture.reads.reset();
    assert!(
        repo.trusted_signers(&member.issuer, 21)
            .await
            .expect("fresh deletion after failed read")
            .is_empty()
    );
    assert_eq!(fixture.reads.totals(), [1, 1]);
    fixture.finish().await;
}
