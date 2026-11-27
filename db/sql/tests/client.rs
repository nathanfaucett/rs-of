#![cfg(all(any(feature = "in-memory", feature = "redb"), feature = "sql"))]

use core::future::pending;
use std::time::Duration;

use futures::executor::block_on;
use ofdb_sql::{
    Database, FromRow, FromRowError, QueryParams, Row, SqlTranslator, Statement, TranslateError,
    Translator, Value,
};

#[cfg(feature = "in-memory")]
#[test]
fn embedded_client_owns_shared_storage_without_tokio() {
    block_on(async {
        let database = Database::in_memory();
        let client = database.client();
        drop(database);

        client
            .execute_sql("CREATE TABLE items (id UUID PRIMARY KEY)", None)
            .await
            .expect("execute without Tokio runtime");
        let rows = client
            .translate_and_execute("SELECT id FROM items", &SqlTranslator)
            .await
            .expect("translate and execute through client");
        assert_eq!(rows.len(), 1);

        let clone = client.clone();
        clone.execute(Vec::new()).await.unwrap_err();
    });
}

#[cfg(all(feature = "in-memory", feature = "sql"))]
#[derive(Debug, PartialEq, Eq)]
struct NamedRow(String);

#[cfg(all(feature = "in-memory", feature = "sql"))]
impl FromRow for NamedRow {
    fn from_row(row: &Row, columns: &[&str]) -> Result<Self, FromRowError> {
        let value = ofdb_sql::decode::<String>(ofdb_sql::value(row, columns, "name")?, "name")?;
        Ok(Self(value))
    }
}

#[cfg(all(feature = "in-memory", feature = "sql"))]
#[test]
#[ignore = "pre-existing disabled test"]
fn embedded_client_exposes_generic_translator_helpers() {
    block_on(async {
        let database = Database::in_memory();
        let client = database.client();
        let shared = database.client();
        client
            .translate_and_execute(
                "CREATE TABLE names (id UUID PRIMARY KEY, name TEXT)",
                &SqlTranslator,
            )
            .await
            .expect("create table through generic helper");
        let params = QueryParams::Positional(vec![Value::from("Ada")]);
        client
            .translate_and_execute_with_params(
                "INSERT INTO names (name) VALUES ($1)",
                Some(&params),
                &SqlTranslator,
            )
            .await
            .expect("execute parameterized statement through client");
        let names = shared
            .translate_and_select::<_, NamedRow>("SELECT name FROM names", &SqlTranslator)
            .await
            .expect("second client sees first client's write");
        assert_eq!(names, vec![NamedRow("Ada".to_owned())]);
    });
}

#[cfg(feature = "redb")]
#[test]
fn redb_client_survives_database_drop_and_persists_data() {
    block_on(async {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let path = directory.path().join("embedded.redb");
        let client = {
            let database = Database::open(&path).expect("open embedded database");
            let client = database.client();
            client
                .execute_sql("CREATE TABLE names (id UUID PRIMARY KEY, name TEXT)", None)
                .await
                .expect("create table through embedded client");
            client
                .execute_sql("INSERT INTO names (name) VALUES ('Ada')", None)
                .await
                .expect("insert through embedded client");
            client
        };
        let results = client
            .execute_sql("SELECT name FROM names", None)
            .await
            .expect("query after dropping database handle");
        assert_eq!(results[0].rows, vec![Row::from(["Ada"])]);
        drop(client);

        let reopened = Database::open(&path).expect("reopen persistent database");
        let results = reopened
            .client()
            .execute_sql("SELECT name FROM names", None)
            .await
            .expect("query persisted row");
        assert_eq!(results[0].rows, vec![Row::from(["Ada"])]);
    });
}

#[cfg(all(feature = "in-memory", feature = "sql"))]
struct PendingTranslator;

#[cfg(all(feature = "in-memory", feature = "sql"))]
impl Translator for PendingTranslator {
    fn translate_with_params(
        &self,
        _query: &str,
        _params: Option<&QueryParams>,
    ) -> impl Future<Output = Result<Vec<Statement>, TranslateError>> {
        pending()
    }
}

#[cfg(all(feature = "in-memory", feature = "sql"))]
#[tokio::test]
async fn deadline_covers_translation_and_is_independent_per_clone() {
    let database = Database::in_memory();
    let base = database.client();
    let timed = base.clone().with_deadline(Duration::from_millis(1));
    let error = timed
        .translate_and_execute("pending", &PendingTranslator)
        .await
        .expect_err("pending translation must time out");
    assert_eq!(error.kind, ofdb_sql::ErrorKind::Timeout);
    base.execute_sql("CREATE TABLE independent (id UUID PRIMARY KEY)", None)
        .await
        .expect("untimed clone keeps its independent deadline");
}

#[cfg(feature = "in-memory")]
#[test]
fn embedded_client_debug_does_not_show_database_data() {
    let client = Database::in_memory().client();
    assert!(format!("{client:?}").contains("Embedded"));
}

#[cfg(feature = "in-memory")]
#[test]
fn explicit_uuid_primary_keys_are_rejected_in_all_insert_forms() {
    use ofdb_sql::{Query, QueryInsert, Uuid};
    use query::{QueryInsertValue, QueryInsertValues};

    block_on(async {
        let client = Database::in_memory().client();
        client
            .execute_sql("CREATE TABLE generated (id UUID PRIMARY KEY)", None)
            .await
            .expect("create table");
        for id in [
            "018f0f8e-7b6d-5c4a-8f12-123456789abc",
            "018f0f8e-7b6d-7c4a-8f12-123456789abc",
        ] {
            let literal = format!("INSERT INTO generated VALUES (CAST('{id}' AS UUID))");
            assert_generated_only(
                &client
                    .execute_sql(&literal, None)
                    .await
                    .expect_err("literal key"),
            );
            let value = Value::Uuid(Uuid::parse_str(id).expect("valid UUID"));
            let params = QueryParams::Positional(vec![value.clone()]);
            assert_generated_only(
                &client
                    .execute_sql("INSERT INTO generated VALUES ($1)", Some(&params))
                    .await
                    .expect_err("bound key"),
            );
            for query in [
                Query::Insert(QueryInsert {
                    table: "generated".into(),
                    row: Row::new(vec![value.clone()]),
                    returning: None,
                }),
                Query::InsertValues(QueryInsertValues {
                    table: "generated".into(),
                    columns: vec!["id".into()],
                    rows: vec![vec![QueryInsertValue::Value(value)]],
                    returning: None,
                    on_conflict_do_nothing: None,
                    on_conflict_do_update: None,
                }),
            ] {
                assert_generated_only(
                    &client
                        .execute(vec![Statement::Query(query)])
                        .await
                        .expect_err("typed key"),
                );
            }
        }
    });
}

#[cfg(feature = "in-memory")]
fn assert_generated_only(error: &ofdb_sql::QueryError) {
    assert_eq!(error.kind, ofdb_sql::QueryErrorKind::Rejected);
    assert!(
        error.message.contains("UUID primary keys are generated"),
        "{error}"
    );
}

#[cfg(feature = "in-memory")]
#[test]
fn generated_keys_are_returned_and_ordinary_uuids_still_work() {
    block_on(async {
        let client = Database::in_memory().client();
        let results = client.execute_sql(
            "CREATE TABLE generated (reference UUID, id UUID PRIMARY KEY); \
             INSERT INTO generated (reference) VALUES (CAST('018f0f8e-7b6d-5c4a-8f12-123456789abc' AS UUID)) RETURNING id; \
             INSERT INTO generated (reference, id) VALUES (CAST('018f0f8e-7b6d-5c4a-8f12-123456789abc' AS UUID), DEFAULT) RETURNING id",
            None,
        ).await.expect("omission and DEFAULT generate keys");
        let keys = [&results[1], &results[2]].map(|result| {
            let key = result.rows[0].values[0].as_uuid().expect("generated key");
            assert_eq!(key.get_version_num(), 7);
            assert_eq!(key.as_bytes()[8] & 0xc0, 0x80);
            result.rows[0].clone()
        });
        assert_ne!(keys[0], keys[1]);
        let rows = client.execute_sql("SELECT id FROM generated WHERE reference = CAST('018f0f8e-7b6d-5c4a-8f12-123456789abc' AS UUID)", None).await.expect("UUID predicate");
        assert_eq!(rows[0].rows.len(), 2);
        assert!(keys.iter().all(|key| rows[0].rows.contains(key)));
    });
}

#[cfg(feature = "in-memory")]
#[test]
fn default_values_inserts_one_row_with_generated_key_and_column_defaults() {
    block_on(async {
        let client = Database::in_memory().client();
        let results = client.execute_sql(
            "CREATE TABLE defaults (label TEXT DEFAULT 'configured', id UUID PRIMARY KEY); \
             INSERT INTO defaults DEFAULT VALUES RETURNING label, id; SELECT label, id FROM defaults",
            None,
        ).await.expect("insert defaults");
        let [row] = results[1].rows.as_slice() else {
            panic!("expected one inserted row")
        };
        assert_eq!(row.values[0], Value::from("configured"));
        assert_eq!(
            row.values[1]
                .as_uuid()
                .expect("generated key")
                .get_version_num(),
            7
        );
        assert_eq!(results[2].rows, results[1].rows);
    });
}

#[cfg(feature = "in-memory")]
#[test]
#[allow(
    clippy::significant_drop_tightening,
    reason = "rollback consumes the transaction"
)]
fn uuid_insert_rejection_precedes_conflicts_and_preserves_atomicity() {
    block_on(async {
        let client = Database::in_memory().client();
        let results = client
            .execute_sql(
                "CREATE TABLE generated (id UUID PRIMARY KEY, label TEXT); \
             INSERT INTO generated (label) VALUES ('original') RETURNING id",
                None,
            )
            .await
            .expect("existing UUIDv7 key");
        let params = QueryParams::Positional(results[1].rows[0].values.clone());
        for sql in [
            "INSERT INTO generated VALUES ($1, 'duplicate')",
            "INSERT INTO generated VALUES ($1, 'ignored') ON CONFLICT (id) DO NOTHING",
            "INSERT INTO generated VALUES ($1, 'updated') ON CONFLICT (id) DO UPDATE SET label = EXCLUDED.label",
            "INSERT INTO generated VALUES (DEFAULT, 'partial'), ($1, 'duplicate')",
            "INSERT INTO generated VALUES (DEFAULT, 42), ($1, 'explicit') ON CONFLICT (id) DO NOTHING",
        ] {
            assert_generated_only(
                &client
                    .execute_sql(sql, Some(&params))
                    .await
                    .expect_err("supplied UUIDv7"),
            );
        }
        let rows = client
            .execute_sql("SELECT label FROM generated", None)
            .await
            .expect("read unchanged rows");
        assert_eq!(rows[0].rows, vec![Row::from(["original"])]);
        client
            .execute_sql("DELETE FROM generated WHERE id = $1", Some(&params))
            .await
            .expect("tombstone the existing row");
        let mut tx = client.transaction().await.expect("begin transaction");
        assert_generated_only(
            &tx.execute_sql(
                "INSERT INTO generated VALUES ($1, 'revived')",
                Some(&params),
            )
            .await
            .expect_err("supplied key precedes tombstone lookup"),
        );
        tx.rollback().await.expect("discard rejected transaction");
        let error = client
            .execute_sql("INSERT INTO generated VALUES (NULL, 'invalid')", None)
            .await
            .expect_err("explicit NULL is not DEFAULT");
        assert_eq!(error.kind, ofdb_sql::QueryErrorKind::Rejected);
        assert!(
            error.message.contains("Primary key must not be NULL"),
            "{error}"
        );
    });
}

#[cfg(all(feature = "in-memory", feature = "validated-history"))]
#[test]
fn fresh_replica_retains_generated_uuid_keys_from_history() {
    use ofdb_sql::history::SyncKey;

    block_on(async {
        let writer = Database::in_memory();
        let reader = Database::in_memory();
        let results = writer
            .client()
            .execute_sql(
                "CREATE TABLE generated (id UUID PRIMARY KEY, label TEXT); \
             INSERT INTO generated (label) VALUES ('peer') RETURNING id, label",
                None,
            )
            .await
            .expect("insert on writer");
        let history = writer.export_history().await.expect("export fresh history");
        let id = *results[1].rows[0].values[0]
            .as_uuid()
            .expect("generated key");
        let user_key = SyncKey::Row {
            table: "generated".into(),
            row: id,
        };
        assert!(history.iter().any(|unit| unit.key == user_key));
        assert!(history.iter().all(|unit| {
            let SyncKey::Row { row, .. } = unit.key;
            row.get_version_num() == 7 && row.as_bytes()[8] & 0xc0 == 0x80
        }));
        reader
            .import_history(history)
            .await
            .expect("restore on fresh Replica");
        let rows = reader
            .client()
            .execute_sql("SELECT id, label FROM generated", None)
            .await
            .expect("read Replica");
        assert_eq!(rows[0].rows, results[1].rows);
    });
}

#[cfg(all(feature = "in-memory", feature = "validated-history"))]
#[tokio::test]
async fn peer_session_preserves_generated_keys_and_applies_later_edits() {
    use history_transport::{Channel, unbounded_channel};
    use ofdb_sql::history::{SessionConfig, SyncRole};
    use tokio::time::timeout;

    let a = Database::in_memory();
    let b = Database::in_memory();
    let staging = tempfile::tempdir().expect("session staging directory");
    let inserted = a
        .client()
        .execute_sql(
            "CREATE TABLE items (id UUID PRIMARY KEY, label TEXT); \
         INSERT INTO items (label) VALUES ('original') RETURNING id",
            None,
        )
        .await
        .expect("insert locally");
    let key = &inserted[1].rows[0];
    assert_eq!(
        key.values[0]
            .as_uuid()
            .expect("generated UUID")
            .get_version_num(),
        7
    );
    let params = QueryParams::Positional(key.values.clone());
    let config = SessionConfig::default();
    for label in ["original", "updated"] {
        if label == "updated" {
            b.client()
                .execute_sql(
                    "UPDATE items SET label = 'updated' WHERE id = $1",
                    Some(&params),
                )
                .await
                .expect("edit on the receiving peer");
        }
        let (a_send, a_recv) = unbounded_channel();
        let (b_send, b_recv) = unbounded_channel();
        let left = Channel(a_send, b_recv);
        let right = Channel(b_send, a_recv);
        let (a_sync, b_sync) = timeout(Duration::from_secs(10), async {
            tokio::join!(
                async {
                    let mut transport = left;
                    a.exchange_resource_history(
                        &mut transport,
                        &config,
                        SyncRole::Initiator,
                        staging.path(),
                        || async { Ok(()) },
                    )
                    .await
                },
                async {
                    let mut transport = right;
                    b.exchange_resource_history(
                        &mut transport,
                        &config,
                        SyncRole::Responder,
                        staging.path(),
                        || async { Ok(()) },
                    )
                    .await
                },
            )
        })
        .await
        .expect("peer session completes");
        assert!(a_sync.is_ok() && b_sync.is_ok(), "{a_sync:?}; {b_sync:?}");
        for peer in [&a, &b] {
            let rows = peer
                .client()
                .execute_sql("SELECT id, label FROM items", None)
                .await
                .expect("query synchronized row");
            assert_eq!(
                rows[0].rows,
                [Row::new(vec![key.values[0].clone(), Value::from(label)])]
            );
        }
    }
}

#[cfg(all(feature = "in-memory", feature = "validated-history"))]
mod history_transport {
    use core::future::ready;

    use ofdb_sql::history::SyncTransport;
    use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

    pub use tokio::sync::mpsc::unbounded_channel;

    pub struct Channel(pub UnboundedSender<Vec<u8>>, pub UnboundedReceiver<Vec<u8>>);

    impl SyncTransport for Channel {
        type Error = &'static str;

        fn send(&mut self, frame: Vec<u8>) -> impl Future<Output = Result<(), Self::Error>> + Send {
            ready(self.0.send(frame).map_err(|_error| "peer closed"))
        }

        async fn receive(&mut self) -> Result<Vec<u8>, Self::Error> {
            self.1.recv().await.ok_or("peer closed")
        }
    }
}

#[cfg(all(feature = "redb", feature = "validated-history"))]
#[tokio::test]
async fn first_upload_rechecks_authorization_after_waiting_for_destination_writer() {
    use core::{
        future::ready,
        sync::atomic::{AtomicBool, Ordering},
    };

    use history_transport::{Channel, unbounded_channel};
    use ofdb_sql::history::{SessionConfig, SyncError, SyncManifest, SyncRole};
    use tokio::time::timeout;

    let directory = tempfile::tempdir().expect("first-upload directory");
    let target = Database::open(directory.path().join("target.redb")).expect("destination");
    let source = Database::in_memory();
    source
        .client()
        .execute_sql(
            "CREATE TABLE items (id UUID PRIMARY KEY); INSERT INTO items DEFAULT VALUES",
            None,
        )
        .await
        .expect("source history");
    let before = source.history_manifest().await.expect("source manifest");
    let client = target.client();
    let writer = client
        .transaction()
        .await
        .expect("hold destination writer without changes");
    let allowed = AtomicBool::new(true);
    let config = SessionConfig::default();
    let (source_send, target_receive) = unbounded_channel();
    let (target_send, source_receive) = unbounded_channel();
    let mut initiator = Channel(source_send, source_receive);
    let mut responder = Channel(target_send, target_receive);
    let check = || {
        ready(if allowed.load(Ordering::Relaxed) {
            Ok(())
        } else {
            Err("revoked".into())
        })
    };
    let (result, ()) = timeout(Duration::from_secs(10), async {
        tokio::join!(
            biased;
            target.receive_first_upload(&mut responder, &config, directory.path(), check, check),
            async {
                source.exchange_resource_history(&mut initiator, &config, SyncRole::Initiator, directory.path(), || ready(Ok(()))).await.expect("SQL exchange with staged destination");
                // The responder runs first and the channel sends never suspend. Once SQL
                // finishes here, publish has already yielded on the held destination writer.
                allowed.store(false, Ordering::Relaxed);
                writer.rollback().await.expect("release unchanged destination writer");
            },
        )
    }).await.expect("first-upload deadline");
    assert!(
        matches!(result, Err(SyncError::PreApply(ref reason)) if reason == "revoked"),
        "{result:?}"
    );
    assert_eq!(
        target
            .history_manifest()
            .await
            .expect("destination manifest"),
        SyncManifest::default()
    );
    assert_eq!(
        source.history_manifest().await.expect("source manifest"),
        before
    );
}

#[cfg(all(feature = "redb", feature = "validated-history"))]
#[tokio::test]
async fn shared_first_upload_rechecks_after_host_writer_and_allows_control_reads() {
    use core::{
        future::ready,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    use futures::poll;
    use history_transport::{Channel, unbounded_channel};
    use ofdb_sql::{
        SharedDatabase,
        history::{
            NativeSyncStageFactory, SessionConfig, SyncError, SyncManifest, SyncRole, SyncStage,
            SyncStageFactory, SyncTransport,
        },
    };
    use tokio::{sync::Notify, time::timeout};

    struct UploadTransport<'a>(Channel, &'a AtomicUsize);
    impl SyncTransport for UploadTransport<'_> {
        type Error = &'static str;

        async fn send(&mut self, frame: Vec<u8>) -> Result<(), Self::Error> {
            // SQL v7 sends a Done after initial transfer and again after recovery.
            if frame == [8] {
                self.1.fetch_add(1, Ordering::Relaxed);
            }
            self.0.send(frame).await
        }

        async fn receive(&mut self) -> Result<Vec<u8>, Self::Error> {
            self.0.receive().await
        }
    }

    let directory = tempfile::tempdir().expect("shared upload directory");
    let host = SharedDatabase::in_memory().expect("shared host");
    let control = Database::open_shared(&host, "control").expect("control namespace");
    let winner = Database::open_shared(&host, "winner").expect("writer namespace");
    let waiting = Database::open_shared(&host, "waiting").expect("destination namespace");
    let source = Database::in_memory();
    source
        .client()
        .execute_sql(
            "CREATE TABLE items (id UUID PRIMARY KEY); INSERT INTO items DEFAULT VALUES",
            None,
        )
        .await
        .expect("source history");
    let before = source.history_manifest().await.expect("source manifest");
    control
        .client()
        .execute_sql(
            "CREATE TABLE policy (id UUID PRIMARY KEY); INSERT INTO policy DEFAULT VALUES",
            None,
        )
        .await
        .expect("policy data");
    let control_read = || async {
        let policy = control
            .client()
            .execute_sql("SELECT id FROM policy", None)
            .await
            .expect("authorization reads the shared host");
        assert_eq!(policy[0].rows.len(), 1);
    };

    let stage_factory =
        NativeSyncStageFactory::shared(host.namespace("stages").expect("stage namespace"));
    let mut abandoned_stage = stage_factory.create().await.expect("independent stage");
    abandoned_stage
        .append(vec![1])
        .await
        .expect("persist staged record");

    let checked = AtomicUsize::new(0);
    let winner_done = AtomicUsize::new(0);
    let waiting_done = AtomicUsize::new(0);
    let waiting_allowed = AtomicBool::new(true);
    let staged = Notify::new();
    let allow_publish = Notify::new();
    let winner_at_commit = Notify::new();
    let release_winner = Notify::new();

    async fn upload<Q, U>(
        source: &Database,
        target: &Database,
        done: &AtomicUsize,
        directory: &std::path::Path,
        check: Q,
    ) -> Result<(), SyncError<&'static str>>
    where
        Q: FnMut() -> U + Send + Copy,
        U: Future<Output = Result<(), String>> + Send,
    {
        let config = SessionConfig::default();
        let (source_send, target_receive) = unbounded_channel();
        let (target_send, source_receive) = unbounded_channel();
        let mut initiator = Channel(source_send, source_receive);
        let mut responder = UploadTransport(Channel(target_send, target_receive), done);
        let (exchange, publish) = tokio::join!(
            source.exchange_resource_history(
                &mut initiator,
                &config,
                SyncRole::Initiator,
                directory,
                || ready(Ok(()))
            ),
            target.receive_first_upload(&mut responder, &config, directory, check, check),
        );
        exchange.expect("SQL exchange with private candidate");
        publish
    }
    let mut waiting_upload = Box::pin(upload(
        &source,
        &waiting,
        &waiting_done,
        directory.path(),
        || async {
            if waiting_done.load(Ordering::Relaxed) == 2
                && checked.fetch_add(1, Ordering::Relaxed) == 0
            {
                staged.notify_one();
                allow_publish.notified().await;
            }
            control_read().await;
            if waiting_allowed.load(Ordering::Relaxed) {
                Ok(())
            } else {
                Err("revoked".into())
            }
        },
    ));
    let winner_checks = AtomicUsize::new(0);
    let mut winner_upload = Box::pin(upload(
        &source,
        &winner,
        &winner_done,
        directory.path(),
        || async {
            if winner_done.load(Ordering::Relaxed) == 2
                && winner_checks.fetch_add(1, Ordering::Relaxed) == 1
            {
                control_read().await;
                winner_at_commit.notify_one();
                release_winner.notified().await;
            }
            Ok(())
        },
    ));

    timeout(Duration::from_secs(10), async {
        tokio::select! {
            () = staged.notified() => {},
            result = &mut waiting_upload => panic!("destination did not stage: {result:?}"),
        }
        tokio::select! {
            () = winner_at_commit.notified() => {},
            result = &mut winner_upload => panic!("winner did not reach its commit check: {result:?}"),
        }
        // Cleanup must not block this executor while another namespace holds the writer.
        drop(abandoned_stage);
        allow_publish.notify_one();
        let pending = poll!(&mut waiting_upload);
        assert!(pending.is_pending(), "publish bypassed the shared writer: {pending:?}");
        let client = control.client();
        let mut regular_write = Box::pin(client.execute_sql("INSERT INTO policy DEFAULT VALUES", None));
        assert!(poll!(&mut regular_write).is_pending(), "ordinary writes must also wait for the shared writer");
        waiting_allowed.store(false, Ordering::Relaxed);
        release_winner.notify_one();
        winner_upload.await.expect("competing writer commits");
        let result = waiting_upload.await;
        regular_write.await.expect("ordinary writer resumes");
        assert!(matches!(result, Err(SyncError::PreApply(ref reason)) if reason == "revoked"), "{result:?}");
    }).await.expect("shared publish and control-read deadline");

    assert_eq!(
        waiting.history_manifest().await.expect("empty destination"),
        SyncManifest::default()
    );
    assert_eq!(
        winner.history_manifest().await.expect("competing history"),
        before
    );
    assert_eq!(
        source.history_manifest().await.expect("unchanged source"),
        before
    );
}
