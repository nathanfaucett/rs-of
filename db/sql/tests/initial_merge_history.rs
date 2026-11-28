#![cfg(all(feature = "sql", feature = "validated-history"))]

use ofdb_sql::{
    Database, ErrorKind, HistoryRecord, Migration, Row, StagedHistory, Value,
    history::{SyncKey, history_revision},
};

const ATOMIC: &[HistoryRecord<'_>] = &[HistoryRecord {
    table: "items",
    identity: "id",
    stable: &[],
    envelope: &["envelope"],
    resolve_by: &[],
}];
const SPLIT: &[HistoryRecord<'_>] = &[HistoryRecord {
    table: "items",
    identity: "id",
    stable: &[],
    envelope: &["ciphertext", "nonce"],
    resolve_by: &[],
}];

const MIGRATED: &[HistoryRecord<'_>] = &[
    ATOMIC[0],
    HistoryRecord {
        table: "_ofdb_migrations",
        identity: "id",
        stable: &[],
        envelope: &["version", "description", "fingerprint", "execution_time"],
        resolve_by: &[],
    },
];

async fn reopen(candidate: &StagedHistory) -> Database {
    let fresh = Database::in_memory();
    fresh
        .import_history(candidate.units().to_vec())
        .await
        .expect("reopen staged history in a fresh database");
    fresh
}

async fn execute(db: &Database, sql: &str) -> Vec<Row> {
    db.client()
        .execute_sql(sql, None)
        .await
        .expect("execute fixture SQL")
        .pop()
        .expect("one statement result")
        .rows
}

async fn stage(source: &Database, target: &Database) {
    target
        .import_history(
            source
                .export_history()
                .await
                .expect("export source history"),
        )
        .await
        .expect("import into private stage");
}

async fn encrypted_rows() -> Database {
    let db = Database::in_memory();
    execute(
        &db,
        "CREATE TABLE items (id TEXT PRIMARY KEY, ciphertext TEXT, nonce TEXT, envelope TEXT, updated_at TEXT)",
    )
    .await;
    db
}

const ITEM_MIGRATION_SQL: &str = "CREATE TABLE items (id TEXT PRIMARY KEY, envelope TEXT)";

fn item_migration() -> [Migration; 1] {
    [Migration::new(1, "create items", ITEM_MIGRATION_SQL)]
}

#[tokio::test]
async fn reconciles_equivalent_independent_migrations_without_dropping_history() {
    let plan = item_migration();
    let left = Database::in_memory();
    let right = Database::in_memory();
    for db in [&left, &right] {
        assert_eq!(
            db.client()
                .migrate(&plan)
                .await
                .expect("apply migration")
                .applied_versions,
            [1]
        );
    }
    let identity = "SELECT id FROM _ofdb_migrations";
    assert_ne!(
        execute(&left, identity).await,
        execute(&right, identity).await
    );
    let definition = "SELECT version, description, fingerprint FROM _ofdb_migrations";
    assert_eq!(
        execute(&left, definition).await,
        execute(&right, definition).await
    );

    execute(&left, "INSERT INTO items VALUES ('left', 'left envelope')").await;
    execute(
        &right,
        "INSERT INTO items VALUES ('right', 'right envelope')",
    )
    .await;
    let left_before = left.history_manifest().await.expect("left baseline");
    let right_before = right.history_manifest().await.expect("right baseline");
    let mut winner = None;
    for (first, second) in [(&left, &right), (&right, &left)] {
        let candidate = first
            .stage_history(second, MIGRATED)
            .await
            .expect("stage independently applied equivalent migrations");
        let reopened = reopen(&candidate).await;
        // A stale source cannot bring back the suppressed copy of a migration row.
        stage(&left, &reopened).await;
        stage(&right, &reopened).await;
        let rows = execute(&reopened, identity).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(
            execute(&reopened, definition).await,
            execute(&left, definition).await
        );
        if let Some(winner) = &winner {
            assert_eq!(winner, &rows);
        } else {
            winner = Some(rows);
        }
        assert_eq!(
            reopened
                .client()
                .migrate(&plan)
                .await
                .expect("reopen with migration runner")
                .applied_versions,
            [] as [u32; 0]
        );
        assert_eq!(
            execute(&reopened, "SELECT id FROM items ORDER BY id").await,
            [
                Row::new(vec![Value::from("left")]),
                Row::new(vec![Value::from("right")])
            ]
        );
        assert_eq!(
            candidate
                .units()
                .iter()
                .filter(|unit| {
                    matches!(&unit.key, SyncKey::Row { table, .. } if table == "_ofdb_migrations")
                })
                .count(),
            2
        );
    }
    assert_eq!(
        left.history_manifest().await.expect("left unchanged"),
        left_before
    );
    assert_eq!(
        right.history_manifest().await.expect("right unchanged"),
        right_before
    );
}

#[tokio::test]
async fn restages_after_remote_publication_and_a_local_edit() {
    let left = Database::in_memory();
    let right = Database::in_memory();
    for source in [&left, &right] {
        source
            .client()
            .migrate(&item_migration())
            .await
            .expect("apply migrations");
    }
    execute(&left, "INSERT INTO items VALUES ('local', 'original')").await;
    execute(&right, "INSERT INTO items VALUES ('remote', 'original')").await;
    let first = left
        .stage_history(&right, MIGRATED)
        .await
        .expect("initial stage");
    let published = reopen(&first).await;
    // Exercise both sides of the canonical ledger choice, including the original loser.
    execute(&left, "UPDATE items SET envelope = 'local edit'").await;
    execute(&right, "UPDATE items SET envelope = 'remote edit'").await;
    for (source, edit) in [(&left, "local edit"), (&right, "remote edit")] {
        let staged = source
            .stage_history(&published, MIGRATED)
            .await
            .expect("fresh stage");
        assert_eq!(first.units().len(), staged.units().len());
        let reopened = reopen(&staged).await;
        assert_eq!(
            execute(&reopened, "SELECT id FROM _ofdb_migrations")
                .await
                .len(),
            1
        );
        assert_eq!(execute(&reopened, "SELECT id FROM items").await.len(), 2);
        assert_eq!(
            execute(
                &reopened,
                "SELECT id FROM items WHERE envelope = 'original'"
            )
            .await
            .len(),
            1
        );
        assert_eq!(
            execute(
                &reopened,
                "SELECT envelope FROM items WHERE envelope != 'original'"
            )
            .await,
            [Row::new(vec![Value::from(edit)])]
        );
    }
}

#[tokio::test]
async fn rejects_divergent_migration_history_under_the_same_id() {
    let plan = item_migration();
    let left = Database::in_memory();
    left.client().migrate(&plan).await.expect("apply plan");
    let left_before = left.history_manifest().await.expect("left baseline");
    let identity = "SELECT id FROM _ofdb_migrations";
    let shared = Database::in_memory();
    stage(&left, &shared).await;
    let candidate = left
        .stage_history(&shared, MIGRATED)
        .await
        .expect("stage shared migration history");
    let reopened = reopen(&candidate).await;
    assert_eq!(
        reopened
            .client()
            .migrate(&plan)
            .await
            .expect("validate staged migrations")
            .applied_versions,
        [] as [u32; 0]
    );

    execute(
        &shared,
        "UPDATE _ofdb_migrations SET description = 'diverged'",
    )
    .await;
    assert_eq!(
        execute(&shared, identity).await,
        execute(&left, identity).await
    );
    let divergent = shared.history_manifest().await.expect("divergent baseline");
    let error = left
        .stage_history(&shared, MIGRATED)
        .await
        .expect_err("reject divergent history under the same UUID");
    assert_eq!(error.kind, ErrorKind::Rejected);
    assert!(
        error.message.contains("Incompatible migration history"),
        "{error}"
    );
    assert_eq!(
        shared
            .history_manifest()
            .await
            .expect("divergent history unchanged"),
        divergent
    );
    assert_eq!(
        left.history_manifest().await.expect("left unchanged"),
        left_before
    );
}

#[tokio::test]
async fn rejects_an_equivalent_looking_ledger_with_reverted_edits() {
    let plan = item_migration();
    let left = Database::in_memory();
    let right = Database::in_memory();
    for source in [&left, &right] {
        source.client().migrate(&plan).await.expect("apply plan");
    }
    execute(
        &right,
        "UPDATE _ofdb_migrations SET description = 'temporary'",
    )
    .await;
    execute(
        &right,
        "UPDATE _ofdb_migrations SET description = 'create items'",
    )
    .await;
    let before = right.history_manifest().await.expect("edited history");
    let error = left
        .stage_history(&right, MIGRATED)
        .await
        .expect_err("reject edited ledger");
    assert!(
        error.message.contains("Incompatible migration history"),
        "{error}"
    );
    assert_eq!(
        right
            .history_manifest()
            .await
            .expect("edited history unchanged"),
        before
    );
}

#[tokio::test]
async fn rejects_reassigned_encrypted_record_even_after_owner_is_restored() {
    let records = [HistoryRecord {
        table: "items",
        identity: "id",
        stable: &["vault_id"],
        envelope: &["ciphertext", "nonce"],
        resolve_by: &[],
    }];
    let left = Database::in_memory();
    execute(
        &left,
        "CREATE TABLE items (id TEXT PRIMARY KEY, vault_id TEXT, ciphertext TEXT, nonce TEXT)",
    )
    .await;
    execute(
        &left,
        "INSERT INTO items VALUES ('item', 'vault-a', 'cipher', 'nonce')",
    )
    .await;
    let right = Database::in_memory();
    stage(&left, &right).await;
    execute(&right, "UPDATE items SET vault_id = 'vault-b'").await;
    execute(&right, "UPDATE items SET vault_id = 'vault-a'").await;
    execute(&right, "DELETE FROM items").await;
    let before = right
        .history_manifest()
        .await
        .expect("capture reassigned record");
    assert_eq!(
        left.stage_history(&right, &records)
            .await
            .expect_err("reject reassigned tombstone")
            .kind,
        ErrorKind::Rejected
    );
    assert_eq!(
        right.history_manifest().await.expect("source unchanged"),
        before
    );
}

#[tokio::test]
async fn a_single_envelope_has_the_same_conflict_winner_in_either_import_order() {
    let left = encrypted_rows().await;
    execute(
        &left,
        "INSERT INTO items (id, envelope) VALUES ('stable', 'original')",
    )
    .await;
    let right = Database::in_memory();
    stage(&left, &right).await;
    execute(
        &left,
        "UPDATE items SET envelope = 'cipher-left:nonce-left', updated_at = '2099'",
    )
    .await;
    execute(
        &right,
        "UPDATE items SET envelope = 'cipher-right:nonce-right', updated_at = '1900'",
    )
    .await;

    let left_first = left
        .stage_history(&right, ATOMIC)
        .await
        .expect("stage same logical row");
    let right_first = right
        .stage_history(&left, ATOMIC)
        .await
        .expect("stage reversed history");
    let merge_left = reopen(&left_first).await;
    let merge_right = reopen(&right_first).await;

    let projection = "SELECT envelope FROM items WHERE id = 'stable'";
    let winner = execute(&merge_left, projection).await;
    let canonical = Database::in_memory();
    stage(&left, &canonical).await;
    stage(&right, &canonical).await;
    assert_eq!(winner, execute(&canonical, projection).await);
    assert_eq!(winner, execute(&merge_right, projection).await);
    assert_eq!(winner.len(), 1);
    assert!(matches!(
        winner[0].values[0].as_text(),
        Some("cipher-left:nonce-left" | "cipher-right:nonce-right")
    ));
    let key = Row::new(vec![Value::from("stable")]);
    assert!(
        merge_left
            .client()
            .row_conflicts("items", &key)
            .await
            .expect("read conflicts")
            .contains(&String::from("envelope"))
    );
}

const RESOLVE_SPLIT: &[HistoryRecord<'_>] = &[HistoryRecord {
    envelope: &["ciphertext", "nonce", "updated_at"],
    resolve_by: &["ciphertext", "nonce"],
    ..SPLIT[0]
}];

async fn shared_envelope() -> (Database, Database) {
    let left = encrypted_rows().await;
    execute(&left, "INSERT INTO items (id, ciphertext, nonce, updated_at) VALUES ('stable', 'original', 'nonce-old', '2000')").await;
    let right = Database::in_memory();
    stage(&left, &right).await;
    (left, right)
}

#[tokio::test]
async fn selects_and_retains_the_complete_canonical_payload_history() {
    let (left, right) = shared_envelope().await;
    execute(
        &left,
        "UPDATE items SET ciphertext = 'left', nonce = 'nonce-left', updated_at = '2099'",
    )
    .await;
    execute(
        &right,
        "UPDATE items SET ciphertext = 'right', nonce = 'nonce-right', updated_at = '1900'",
    )
    .await;
    let projection = "SELECT ciphertext, nonce, updated_at FROM items";
    let canonical = Database::in_memory();
    stage(&left, &canonical).await;
    stage(&right, &canonical).await;
    let ciphertext = execute(&canonical, "SELECT ciphertext FROM items").await;
    let winner = if ciphertext == execute(&left, "SELECT ciphertext FROM items").await {
        &left
    } else {
        &right
    };
    let expected = execute(winner, projection).await;

    let forward = left
        .stage_history(&right, RESOLVE_SPLIT)
        .await
        .expect("resolve envelope");
    let reversed = right
        .stage_history(&left, RESOLVE_SPLIT)
        .await
        .expect("resolve in reverse order");
    assert_eq!(
        history_revision(forward.units()),
        history_revision(reversed.units())
    );
    let reopened = reopen(&forward).await;
    assert_eq!(execute(&reopened, projection).await, expected);
    let resolved = reopened
        .history_manifest()
        .await
        .expect("resolved manifest");
    for source in [&left, &right] {
        stage(source, &reopened).await;
        assert_eq!(
            reopened
                .history_manifest()
                .await
                .expect("history after replay"),
            resolved
        );
    }
    let candidate_revision = reopened
        .history_revision()
        .await
        .expect("candidate revision");
    for source in [&left, &right] {
        source
            .publish_local_history(
                &reopened,
                source.history_revision().await.expect("source revision"),
                candidate_revision,
                vec![1],
                || Ok(()),
            )
            .await
            .expect("publish exact candidate over either branch");
        assert_eq!(execute(source, projection).await, expected);
        assert_eq!(
            source.history_revision().await.expect("published revision"),
            candidate_revision
        );
    }
    let key = Row::new(vec![Value::from("stable")]);
    assert_eq!(
        reopened
            .client()
            .row_conflicts("items", &key)
            .await
            .expect("read conflicts"),
        [] as [String; 0]
    );
}

#[tokio::test]
async fn rejects_partial_or_ambiguous_source_groups() {
    for (left_edit, right_edit, reason) in [
        (
            "ciphertext = 'left'",
            "ciphertext = 'right', nonce = 'nonce-right'",
            "Incomplete source",
        ),
        (
            "ciphertext = 'same', nonce = 'nonce-left'",
            "ciphertext = 'same', nonce = 'nonce-right'",
            "Ambiguous canonical source",
        ),
        (
            "ciphertext = 'left', nonce = NULL",
            "ciphertext = 'right', nonce = 'nonce-right'",
            "Incomplete source",
        ),
    ] {
        let (left, right) = shared_envelope().await;
        execute(&left, &format!("UPDATE items SET {left_edit}")).await;
        execute(&right, &format!("UPDATE items SET {right_edit}")).await;
        for (first, second) in [(&left, &right), (&right, &left)] {
            let error = first
                .stage_history(second, RESOLVE_SPLIT)
                .await
                .expect_err("reject unverifiable envelope");
            assert_eq!(error.kind, ErrorKind::Rejected);
            assert!(error.message.contains(reason), "{error}");
        }
    }
}

#[tokio::test]
async fn rejects_ciphertext_and_nonce_combined_from_different_sources() {
    let left = encrypted_rows().await;
    execute(
        &left,
        "INSERT INTO items (id, ciphertext, nonce) VALUES ('stable', 'cipher-old', 'nonce-old')",
    )
    .await;
    let right = Database::in_memory();
    stage(&left, &right).await;
    execute(&left, "UPDATE items SET ciphertext = 'cipher-left'").await;
    execute(&right, "UPDATE items SET nonce = 'nonce-right'").await;

    let error = left
        .stage_history(&right, SPLIT)
        .await
        .expect_err("reject mixed tuple");
    assert_eq!(error.kind, ErrorKind::Rejected);
    assert!(error.message.contains("did not come from one source row"));
    let projection = "SELECT ciphertext, nonce FROM items WHERE id = 'stable'";
    assert_eq!(
        execute(&left, projection).await,
        [Row::new(vec![
            Value::from("cipher-left"),
            Value::from("nonce-old")
        ])]
    );
    assert_eq!(
        execute(&right, projection).await,
        [Row::new(vec![
            Value::from("cipher-old"),
            Value::from("nonce-right")
        ])]
    );
}

#[tokio::test]
async fn a_deleted_logical_row_survives_reimport_of_stale_history() {
    let live = encrypted_rows().await;
    execute(
        &live,
        "INSERT INTO items (id, envelope) VALUES ('stable', 'cipher:nonce')",
    )
    .await;
    let deleted = Database::in_memory();
    stage(&live, &deleted).await;
    execute(&deleted, "DELETE FROM items WHERE id = 'stable'").await;

    execute(&live, "UPDATE items SET envelope = 'concurrent-edit'").await;
    for (first, second) in [(&deleted, &live), (&live, &deleted)] {
        let candidate = first
            .stage_history(second, ATOMIC)
            .await
            .expect("stage delete and edit");
        let reopened = reopen(&candidate).await;
        stage(&live, &reopened).await;
        assert_eq!(execute(&reopened, "SELECT id FROM items").await, []);
        assert_tombstone(&live, &candidate).await;
    }
    assert_eq!(
        execute(&live, "SELECT id FROM items").await,
        [Row::new(vec![Value::from("stable")])]
    );
    assert_eq!(execute(&deleted, "SELECT id FROM items").await, []);
}

async fn assert_tombstone(live: &Database, candidate: &StagedHistory) {
    let original = live.export_history().await.expect("export live history");
    let live_item = original
        .iter()
        .find(|unit| matches!(&unit.key, SyncKey::Row { table, .. } if table == "items"))
        .expect("item history");
    let deleted_item = candidate
        .units()
        .iter()
        .find(|unit| unit.key == live_item.key)
        .expect("same logical row retained");
    assert_ne!(deleted_item.metadata, live_item.metadata);
}

#[tokio::test]
async fn rejects_independent_identity_collisions_even_if_one_claim_is_deleted() {
    let left = encrypted_rows().await;
    let right = encrypted_rows().await;
    execute(
        &left,
        "INSERT INTO items (id, envelope) VALUES ('stable', 'left')",
    )
    .await;
    execute(
        &right,
        "INSERT INTO items (id, envelope) VALUES ('stable', 'right')",
    )
    .await;
    let projection = "SELECT envelope FROM items WHERE id = 'stable'";

    for delete in [false, true] {
        if delete {
            execute(&right, "DELETE FROM items WHERE id = 'stable'").await;
        }
        let before = right.history_manifest().await.expect("right baseline");
        for (first, second) in [(&left, &right), (&right, &left)] {
            let error = first
                .stage_history(second, ATOMIC)
                .await
                .expect_err("reject collision");
            assert!(error.message.contains("identity collision"), "{error}");
        }
        assert_eq!(
            right.history_manifest().await.expect("right unchanged"),
            before
        );
    }
    assert_eq!(
        execute(&left, projection).await,
        [Row::new(vec![Value::from("left")])]
    );
    assert_eq!(execute(&right, projection).await, []);
}

#[tokio::test]
async fn captures_independent_records_without_writing_or_following_either_source() {
    let left = encrypted_rows().await;
    let right = encrypted_rows().await;
    execute(
        &left,
        "INSERT INTO items (id, ciphertext, nonce) VALUES ('left', 'left-cipher', 'left-nonce')",
    )
    .await;
    execute(
        &right,
        "INSERT INTO items (id, ciphertext, nonce) VALUES ('right', 'right-cipher', 'right-nonce')",
    )
    .await;
    let left_before = left.history_manifest().await.expect("left baseline");
    let right_before = right.history_manifest().await.expect("right baseline");
    let candidate = left
        .stage_history(&right, SPLIT)
        .await
        .expect("stage independent records");
    assert_eq!(
        left.history_manifest().await.expect("left unchanged"),
        left_before
    );
    assert_eq!(
        right.history_manifest().await.expect("right unchanged"),
        right_before
    );
    execute(&left, "DELETE FROM items WHERE id = 'left'").await;
    execute(
        &right,
        "UPDATE items SET ciphertext = 'new-cipher', nonce = 'new-nonce'",
    )
    .await;
    assert_eq!(
        execute(
            &reopen(&candidate).await,
            "SELECT id, ciphertext, nonce FROM items ORDER BY id"
        )
        .await,
        [
            Row::new(vec![
                Value::from("left"),
                Value::from("left-cipher"),
                Value::from("left-nonce")
            ]),
            Row::new(vec![
                Value::from("right"),
                Value::from("right-cipher"),
                Value::from("right-nonce")
            ])
        ],
    );
}

#[tokio::test]
async fn rejects_ambiguous_split_envelopes_and_incompatible_schemas() {
    let left = encrypted_rows().await;
    execute(
        &left,
        "INSERT INTO items (id, ciphertext, nonce) VALUES ('stable', 'old-cipher', 'old-nonce')",
    )
    .await;
    let right = Database::in_memory();
    stage(&left, &right).await;
    execute(
        &left,
        "UPDATE items SET ciphertext = 'left-cipher', nonce = 'left-nonce'",
    )
    .await;
    execute(
        &right,
        "UPDATE items SET ciphertext = 'right-cipher', nonce = 'right-nonce'",
    )
    .await;
    let error = left
        .stage_history(&right, SPLIT)
        .await
        .expect_err("reject split conflicts");
    assert!(error.message.contains("column conflict"), "{error}");
    execute(&right, "CREATE TABLE unvalidated (id TEXT PRIMARY KEY)").await;
    let error = left
        .stage_history(&right, SPLIT)
        .await
        .expect_err("reject unlisted table");
    assert!(
        error.message.contains("do not match record policy"),
        "{error}"
    );
    let incompatible = Database::in_memory();
    execute(
        &incompatible,
        "CREATE TABLE items (id TEXT PRIMARY KEY, ciphertext TEXT, nonce TEXT)",
    )
    .await;
    let error = left
        .stage_history(&incompatible, SPLIT)
        .await
        .expect_err("reject a different layout");
    assert!(
        error.message.contains("Incompatible history table schemas"),
        "{error}"
    );
}

#[cfg(feature = "redb")]
#[tokio::test]
async fn includes_the_authentication_proof_context_in_envelope_provenance() {
    let directory = tempfile::tempdir().expect("create test directory");
    let path = directory.path().join("vault.redb");
    let left = Database::open(&path).expect("open local vault database");
    execute(&left, "CREATE TABLE vaults (id TEXT PRIMARY KEY, authentication TEXT, salt TEXT, iv TEXT, created_at TEXT)").await;
    execute(
        &left,
        "INSERT INTO vaults VALUES ('vault', 'proof', 'salt', 'iv', 'original')",
    )
    .await;
    let right = Database::in_memory();
    stage(&left, &right).await;
    execute(&left, "UPDATE vaults SET authentication = 'new-proof'").await;
    execute(
        &right,
        "UPDATE vaults SET salt = 'other-salt', iv = 'other-iv', created_at = 'other-time'",
    )
    .await;

    let records = [HistoryRecord {
        table: "vaults",
        identity: "id",
        stable: &[],
        envelope: &["authentication", "salt", "iv", "created_at"],
        resolve_by: &[],
    }];
    let before = left.history_manifest().await.expect("local baseline");
    let error = left
        .stage_history(&right, &records)
        .await
        .expect_err("reject a proof paired with context from another source");
    assert!(
        error.message.contains("did not come from one source row"),
        "{error}"
    );
    drop(left);
    let reopened = Database::open_existing(&path).expect("reopen local vault database");
    assert_eq!(
        reopened
            .history_manifest()
            .await
            .expect("local history unchanged"),
        before
    );
}
