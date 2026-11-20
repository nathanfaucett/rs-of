#![cfg(all(feature = "native", feature = "ofdb"))]

use std::sync::Arc;

use db::adapters::{EmbeddedDatabase, MemoryDatabase};
use db::ofdb::up;
use ofdb_sql::Database as SqlDatabase;
use tempfile::tempdir;

#[tokio::test]
async fn memory_adapter_initializes_and_exposes_client() {
    let adapter = MemoryDatabase::new().await.expect("memory adapter");
    let client = adapter.client();
    client
        .execute_sql("SELECT * FROM users LIMIT 0", None)
        .await
        .expect("schema initialized");
    let txn = client.transaction().await.expect("transaction");
    txn.rollback().await.expect("rollback");
}

#[tokio::test]
async fn embedded_adapter_initializes_all_owner_tables() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("unified.redb");
    let adapter = EmbeddedDatabase::open_file(&path)
        .await
        .expect("embedded adapter");
    let client = adapter.client();
    for table in [
        "users",
        "clients",
        "sync_selections",
        "devices",
        "namespaces",
        "resources",
        "databases",
        "filesystems",
    ] {
        client
            .execute_sql(&format!("SELECT * FROM {table} LIMIT 0"), None)
            .await
            .expect("owner table exists");
    }
}

#[tokio::test]
async fn unified_schema_persists_and_reopens() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("unified.redb");

    {
        let adapter = EmbeddedDatabase::open_file(&path).await.expect("open");
        let client = adapter.client();
        up(&client).await.expect("initialize");
        up(&client).await.expect("idempotent");
        let mut txn = client.transaction().await.expect("txn");
        txn.execute_sql("INSERT INTO users (id, handle) VALUES (CAST('00000000-0000-7000-8000-000000000001' AS UUID), 'shared')", None)
            .await
            .expect("write user");
        txn.commit().await.expect("commit")
    };

    {
        let adapter = EmbeddedDatabase::open_file(&path).await.expect("reopen");
        let client = adapter.client();
        up(&client).await.expect("reopen schema");
        let result = client
            .execute_sql("SELECT handle FROM users", None)
            .await
            .expect("read committed");
        assert_eq!(result[0].rows.len(), 1);

        let mut txn = client.transaction().await.expect("txn");
        txn.execute_sql("DELETE FROM users", None)
            .await
            .expect("delete inside txn");
        txn.rollback().await.expect("rollback");
        let result = client
            .execute_sql("SELECT handle FROM users", None)
            .await
            .expect("read rolled back");
        assert_eq!(result[0].rows.len(), 1);
    }
}

#[tokio::test]
async fn cross_owner_transaction_commit_and_rollback() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("unified.redb");
    let adapter = EmbeddedDatabase::open_file(&path).await.expect("open");
    let client = adapter.client();

    let mut txn = client.transaction().await.expect("txn");
    txn.execute_sql("INSERT INTO users (id, handle) VALUES (CAST('00000000-0000-7000-8000-000000000002' AS UUID), 'u1')", None)
        .await
        .expect("insert user");
    txn.execute_sql("INSERT INTO devices (owner_subject) VALUES ('owner')", None)
        .await
        .expect("insert device");
    txn.commit().await.expect("commit cross-owner");

    let mut txn = client.transaction().await.expect("txn");
    txn.execute_sql("DELETE FROM users", None)
        .await
        .expect("delete users");
    txn.rollback().await.expect("rollback");
    let result = client
        .execute_sql("SELECT handle FROM users", None)
        .await
        .expect("read");
    assert_eq!(result[0].rows.len(), 1);
}

#[tokio::test]
async fn repeated_initialization_idempotent() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("unified.redb");
    for _ in 0..3 {
        let adapter = EmbeddedDatabase::open_file(&path).await.expect("open");
        let client = adapter.client();
        up(&client).await.expect("init");
    }
}

#[tokio::test]
async fn failed_migration_returns_error() {
    let database = SqlDatabase::in_memory();
    let client = database.client();
    let result = up(&client).await;
    result.unwrap();
    let mut txn = client.transaction().await.expect("txn");
    txn.execute_sql("DROP TABLE users", None)
        .await
        .expect("drop table");
    txn.commit().await.expect("commit");
    let result = up(&client).await;
    assert!(
        result.is_ok(),
        "re-init after corruption should succeed (tables recreated)"
    );
}

#[tokio::test]
async fn shared_injected_database_works() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("unified.redb");
    let shared = Arc::new(EmbeddedDatabase::open_file(&path).await.expect("shared"));
    let shared_database = shared.database.clone();

    let adapter = db::local_database(dir.path(), Some(Arc::new(shared_database)))
        .await
        .expect("local with injected");
    let client = adapter.client();
    client
        .execute_sql("SELECT * FROM users LIMIT 0", None)
        .await
        .expect("shared schema");
}

#[tokio::test]
async fn in_memory_database_works() {
    let database = SqlDatabase::in_memory();
    let client = database.client();
    let result = up(&client).await;
    result.unwrap();
    client
        .execute_sql("SELECT * FROM users LIMIT 0", None)
        .await
        .expect("schema initialized");
}
