use embedded_db::{EmbeddedDatabase, MemoryDatabase};
use of_model::db::Database;

#[tokio::test]
async fn memory_databases_are_migrated_isolated_and_shared_by_clones() {
    let database = MemoryDatabase::new()
        .await
        .expect("initialize memory schema");
    let clone = database.clone();
    let shared: &dyn Database = &database;
    shared
        .idp()
        .client()
        .execute_sql(INSERT_USER, None)
        .await
        .expect("insert memory user");
    let result = Database::client(&clone)
        .execute_sql("SELECT handle FROM users", None)
        .await
        .expect("read through clone");
    assert_eq!(
        result[0].rows,
        vec![db::Row::new(vec![db::Value::Text("retained".into())])]
    );
    let isolated = MemoryDatabase::new()
        .await
        .expect("independent memory database");
    let result = Database::client(&isolated)
        .execute_sql("SELECT handle FROM users", None)
        .await
        .expect("read isolated database");
    assert!(result[0].rows.is_empty());
}

#[tokio::test]
async fn owner_views_share_schema_and_atomic_transactions_on_both_local_backends() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    {
        let embedded = EmbeddedDatabase::open(directory.path())
            .await
            .expect("embedded schema");
        let memory = MemoryDatabase::new().await.expect("memory schema");
        for database in [&embedded as &dyn Database, &memory as &dyn Database] {
            check_owner_operations(database).await;
        }
    }
    let reopened = EmbeddedDatabase::open(directory.path())
        .await
        .expect("reopen all owner data");
    check_owner_values(&reopened).await;
}

async fn check_owner_values(database: &dyn Database) {
    for (client, table, column) in [
        (database.idp().client(), "users", "handle"),
        (database.device().client(), "devices", "name"),
        (database.storage().client(), "namespaces", "owner_subject"),
        (
            database.management().client(),
            "sync_selections",
            "owner_subject",
        ),
    ] {
        let result = client
            .execute_sql(&format!("SELECT {column} FROM {table}"), None)
            .await
            .expect("owner read");
        assert_eq!(
            result[0].rows,
            vec![db::Row::new(vec![db::Value::Text("shared".into())])],
            "{table}"
        );
    }
}

async fn check_owner_operations(database: &dyn Database) {
    for table in [
        "users",
        "clients",
        "devices",
        "local_device_enrollments",
        "device_selection_policies",
        "device_resource_selections",
        "sync_selections",
        "namespaces",
        "resources",
        "databases",
        "filesystems",
    ] {
        database
            .client()
            .execute_sql(&format!("SELECT * FROM {table} LIMIT 0"), None)
            .await
            .expect("complete owner schema");
    }
    let id = "CAST('00000000-0000-7000-8000-000000000002' AS UUID)";
    let mut transaction = database.txn().await.expect("cross-owner transaction");
    for sql in [
        format!("INSERT INTO users (id, handle) VALUES ({id}, 'shared')"),
        format!("INSERT INTO devices (id, owner_subject, name) VALUES ({id}, 'shared', 'shared')"),
        format!(
            "INSERT INTO namespaces (id, owner_subject, application_id) VALUES ({id}, 'shared', {id})"
        ),
        format!(
            "INSERT INTO sync_selections (id, device_id, resource_id, application_id, owner_subject, selected_at) VALUES ({id}, {id}, {id}, {id}, 'shared', 1)"
        ),
    ] {
        transaction
            .execute_sql(&sql, None)
            .await
            .expect("write owner record");
    }
    transaction.commit().await.expect("commit all owners");
    check_owner_values(database).await;
    for (client, table, column) in [
        (database.idp().client(), "users", "handle"),
        (database.device().client(), "devices", "name"),
        (database.storage().client(), "namespaces", "owner_subject"),
        (
            database.management().client(),
            "sync_selections",
            "owner_subject",
        ),
    ] {
        client
            .execute_sql(&format!("UPDATE {table} SET {column} = 'updated'"), None)
            .await
            .expect("owner update");
        let result = client
            .execute_sql(&format!("SELECT {column} FROM {table}"), None)
            .await
            .expect("read update");
        assert_eq!(
            result[0].rows,
            vec![db::Row::new(vec![db::Value::Text("updated".into())])]
        );
        client
            .execute_sql(&format!("UPDATE {table} SET {column} = 'shared'"), None)
            .await
            .expect("restore value");
    }
    let mut transaction = database.txn().await.expect("cross-owner rollback");
    for table in ["users", "devices", "namespaces", "sync_selections"] {
        transaction
            .execute_sql(&format!("DELETE FROM {table}"), None)
            .await
            .expect("delete owner record");
    }
    transaction.rollback().await.expect("rollback all owners");
    check_owner_values(database).await;
}

#[tokio::test]
async fn embedded_open_reports_directory_and_redb_errors_as_database_errors() {
    let root = tempfile::tempdir().expect("temporary directory");
    let file = root.path().join("not-a-directory");
    std::fs::write(&file, b"file").expect("write directory conflict");
    let error = match EmbeddedDatabase::open(&file).await {
        Ok(_) => panic!("file cannot be a data directory"),
        Err(error) => error,
    };
    let _: &of_model::error::DatabaseError = &error;
    assert_eq!(error.kind, db::QueryErrorKind::Storage);
    let path = root.path().join("unified.redb");
    std::fs::write(&path, b"not a redb file").expect("write corrupt database");
    assert!(EmbeddedDatabase::open(root.path()).await.is_err());
    assert_eq!(
        std::fs::read(path).expect("retained invalid data"),
        b"not a redb file"
    );
}

#[tokio::test]
async fn embedded_open_returns_migration_errors_without_partial_owner_tables() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("unified.redb");
    {
        let database = db::Database::open(&path).expect("prepare incompatible database");
        database
            .client()
            .execute_sql("CREATE TABLE devices (id UUID PRIMARY KEY)", None)
            .await
            .expect("incompatible Device schema");
    }
    assert!(EmbeddedDatabase::open(directory.path()).await.is_err());
    let database = db::Database::open(&path).expect("reopen after failed migration");
    database
        .client()
        .execute_sql("SELECT id FROM devices", None)
        .await
        .expect("original table remains");
    assert!(
        database
            .client()
            .execute_sql("SELECT * FROM users", None)
            .await
            .is_err()
    );
}

const INSERT_USER: &str = "INSERT INTO users (id, handle) VALUES (CAST('00000000-0000-7000-8000-000000000001' AS UUID), 'retained')";

#[tokio::test]
async fn embedded_open_migrates_and_retains_committed_data_after_reopen() {
    let root = tempfile::tempdir().expect("temporary directory");
    let directory = root.path().join("nested/data");
    {
        let database = EmbeddedDatabase::open(&directory)
            .await
            .expect("open and migrate");
        let shared: &dyn Database = &database;
        let mut transaction = shared.txn().await.expect("shared transaction");
        transaction
            .execute_sql(INSERT_USER, None)
            .await
            .expect("insert user");
        transaction.commit().await.expect("commit user");
    }
    assert!(directory.join("unified.redb").is_file());
    assert_eq!(
        std::fs::read_dir(&directory)
            .expect("database files")
            .count(),
        1
    );
    let database = EmbeddedDatabase::open(&directory)
        .await
        .expect("reopen and repeat migration");
    let shared: &dyn Database = &database;
    let result = shared
        .management()
        .client()
        .execute_sql("SELECT handle FROM users", None)
        .await
        .expect("read retained user through another owner");
    assert_eq!(
        result[0].rows,
        vec![db::Row::new(vec![db::Value::Text("retained".into())])]
    );
}
