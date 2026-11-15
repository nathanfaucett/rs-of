#![cfg(feature = "replica")]

use futures::executor::block_on;
use of_model::db::Database;
use of_model::idp;
use of_model::replica;

#[test]
fn query_client_initializes_memory_database_through_the_shared_interface() {
    block_on(async {
        let database = ofdb_sql::Database::in_memory();
        let client = database.client();
        let shared: &dyn Database = &client;
        replica::up(shared)
            .await
            .expect("initialize through query client");
        shared
            .storage()
            .client()
            .execute_sql("SELECT * FROM resources LIMIT 0", None)
            .await
            .expect("Storage catalog through owner view");
        let transaction = shared.txn().await.expect("query-client transaction");
        transaction
            .rollback()
            .await
            .expect("rollback query-client transaction");
    });
}

#[test]
fn single_migration_initializes_all_owner_tables_through_each_entry_point() {
    block_on(async {
        let identity = ofdb_sql::Database::in_memory();
        idp::replica::up(&identity)
            .await
            .expect("identity entry point applies the shared schema");
        let device = ofdb_sql::Database::in_memory();
        replica::up_device(&device.client())
            .await
            .expect("Device entry point applies the shared schema");
        for database in [&identity, &device] {
            let client = database.client();
            for table in ["users", "devices", "namespaces"] {
                client
                    .execute_sql(&format!("SELECT * FROM {table} LIMIT 0"), None)
                    .await
                    .expect("owner table exists");
            }
        }
    });
}

#[test]
fn unified_schema_initializes_redb_and_keeps_committed_data_on_reopen() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let path = directory.path().join("unified.redb");
    block_on(async {
        let database = ofdb_sql::Database::open(&path).expect("open redb");
        replica::up(&database).await.expect("initialize all owners");
        replica::up(&database)
            .await
            .expect("idempotent initialization");
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
                .idp()
                .client()
                .execute_sql(&format!("SELECT * FROM {table} LIMIT 0"), None)
                .await
                .expect("owner schema exists");
        }
        let mut transaction = database.txn().await.expect("start shared transaction");
        transaction.execute_sql("INSERT INTO users (id, handle) VALUES (CAST('00000000-0000-7000-8000-000000000001' AS UUID), 'shared')", None).await.expect("write user");
        transaction
            .commit()
            .await
            .expect("commit shared transaction");
    });
    block_on(async {
        let database = ofdb_sql::Database::open(&path).expect("reopen redb");
        replica::up(&database).await.expect("reopen schema");
        let result = database
            .management()
            .client()
            .execute_sql("SELECT handle FROM users", None)
            .await
            .expect("read committed user through another owner");
        assert_eq!(result[0].rows.len(), 1);
        let mut transaction = database.txn().await.expect("start rollback transaction");
        transaction
            .execute_sql("DELETE FROM users", None)
            .await
            .expect("delete inside transaction");
        transaction
            .rollback()
            .await
            .expect("rollback shared transaction");
        let result = database
            .device()
            .client()
            .execute_sql("SELECT handle FROM users", None)
            .await
            .expect("read rolled back user");
        assert_eq!(result[0].rows.len(), 1);
    });
}
