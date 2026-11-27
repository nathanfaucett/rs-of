#![cfg(all(feature = "sql", feature = "validated-history"))]

use ofdb_sql::{Database, Row, Value, history::SyncKey};

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

    let merge_left = Database::in_memory();
    stage(&left, &merge_left).await;
    stage(&right, &merge_left).await;
    let merge_right = Database::in_memory();
    stage(&right, &merge_right).await;
    stage(&left, &merge_right).await;

    let projection = "SELECT envelope FROM items WHERE id = 'stable'";
    let winner = execute(&merge_left, projection).await;
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

#[tokio::test]
async fn per_column_history_can_mix_a_legacy_ciphertext_and_nonce() {
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

    let staged = Database::in_memory();
    stage(&left, &staged).await;
    stage(&right, &staged).await;
    let projection = "SELECT ciphertext, nonce FROM items WHERE id = 'stable'";
    assert_eq!(
        execute(&staged, projection).await,
        [Row::new(vec![
            Value::from("cipher-left"),
            Value::from("nonce-right")
        ])]
    );
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

    let staged = Database::in_memory();
    stage(&deleted, &staged).await;
    stage(&live, &staged).await;
    assert!(execute(&staged, "SELECT id FROM items").await.is_empty());
    let retained = staged
        .export_history()
        .await
        .expect("export staged history");
    let original = live.export_history().await.expect("export live history");
    let live_item = original
        .iter()
        .find(|unit| matches!(&unit.key, SyncKey::Row { table, .. } if table == "items"))
        .expect("item history");
    let deleted_item = retained
        .iter()
        .find(|unit| unit.key == live_item.key)
        .expect("same logical row retained");
    assert_ne!(deleted_item.metadata, live_item.metadata);
    assert_eq!(
        execute(&live, "SELECT id FROM items").await,
        [Row::new(vec![Value::from("stable")])]
    );
}

#[tokio::test]
async fn an_independent_natural_key_collision_can_hide_a_logical_row() {
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

    let staged = Database::in_memory();
    stage(&left, &staged).await;
    stage(&right, &staged).await;
    assert_eq!(execute(&staged, projection).await.len(), 1);
    assert_eq!(
        staged
            .export_history()
            .await
            .expect("export staged history")
            .iter()
            .filter(|unit| matches!(&unit.key, SyncKey::Row { table, .. } if table == "items"))
            .count(),
        2
    );
    assert_eq!(
        execute(&left, projection).await,
        [Row::new(vec![Value::from("left")])]
    );
    assert_eq!(
        execute(&right, projection).await,
        [Row::new(vec![Value::from("right")])]
    );
}
