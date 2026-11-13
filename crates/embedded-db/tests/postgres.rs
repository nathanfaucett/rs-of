#![cfg(feature = "postgres")]

use embedded_db::NetworkDatabase;
use of_model::db::Database;

#[tokio::test]
async fn requires_explicit_plaintext_connection_policy() {
    let result = NetworkDatabase::connect("host=localhost dbname=unused").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_postgres_configuration_is_a_validation_error() {
    let error = match NetworkDatabase::connect("not a connection string").await {
        Ok(_) => panic!("reject invalid PostgreSQL configuration"),
        Err(error) => error,
    };
    assert_eq!(error.kind, db::QueryErrorKind::Validation);
}

#[tokio::test]
async fn compiles_all_owner_tables_without_postgres_credentials() {
    use ofdb_sql::{PostgresStatement, SqlTranslator, Translator};
    let statements = SqlTranslator
        .translate(of_model::replica::MIGRATION)
        .await
        .expect("parse unified schema");
    assert!(statements.len() > 30);
    for statement in statements {
        PostgresStatement::translate(&statement).expect("compile owner schema for PostgreSQL");
    }
}

#[tokio::test]
#[ignore = "requires OF_TEST_POSTGRES_URL for a disposable PostgreSQL database"]
async fn postgres_shared_owner_transactions() {
    let url = std::env::var("OF_TEST_POSTGRES_URL")
        .expect("set OF_TEST_POSTGRES_URL with sslmode=disable");
    let database = NetworkDatabase::connect(&url)
        .await
        .expect("connect and migrate real PostgreSQL");
    let shared: &dyn Database = &database;
    assert!(shared.local_storage().is_none());
    let id = db::Uuid::now_v7();
    let params = db::QueryParams::Positional(vec![db::Value::Uuid(id)]);
    let mut transaction = shared.txn().await.expect("begin shared transaction");
    transaction
        .execute_sql(
            "INSERT INTO users (id, handle) VALUES ($1, 'postgres-test')",
            Some(&params),
        )
        .await
        .expect("write IdP");
    transaction
        .execute_sql(
            "INSERT INTO devices (id, owner_subject) VALUES ($1, 'postgres-test')",
            Some(&params),
        )
        .await
        .expect("write Device");
    transaction.rollback().await.expect("rollback owners");
    assert!(
        shared
            .idp()
            .client()
            .execute_sql("SELECT id FROM users WHERE id = $1", Some(&params))
            .await
            .expect("read rollback")[0]
            .rows
            .is_empty()
    );
    let mut transaction = shared.txn().await.expect("begin commit");
    transaction
        .execute_sql("INSERT INTO users (id) VALUES ($1)", Some(&params))
        .await
        .expect("write IdP");
    transaction
        .execute_sql(
            "INSERT INTO devices (id, owner_subject) VALUES ($1, 'postgres-test')",
            Some(&params),
        )
        .await
        .expect("write Device");
    transaction
        .execute_sql(
            "INSERT INTO namespaces (id, owner_subject, application_id) VALUES ($1, 'postgres-test', $1); INSERT INTO sync_selections (id, device_id, resource_id, application_id, owner_subject, selected_at) VALUES ($1, $1, $1, $1, 'postgres-test', 1)",
            Some(&params),
        )
        .await
        .expect("write Storage and Management");
    transaction.commit().await.expect("commit owners");
    let reopened = NetworkDatabase::connect(&url)
        .await
        .expect("repeat migration");
    assert_eq!(
        reopened
            .device()
            .client()
            .execute_sql("SELECT id FROM devices WHERE id = $1", Some(&params))
            .await
            .expect("read persisted owner")[0]
            .rows[0]
            .values[0],
        db::Value::Uuid(id)
    );
    for (client, table, column) in [
        (reopened.idp().client(), "users", "handle"),
        (reopened.device().client(), "devices", "owner_subject"),
        (reopened.storage().client(), "namespaces", "owner_subject"),
        (
            reopened.management().client(),
            "sync_selections",
            "owner_subject",
        ),
    ] {
        let result = client
            .execute_sql(
                &format!(
                    "UPDATE {table} SET {column} = 'updated' WHERE id = $1 RETURNING {column}"
                ),
                Some(&params),
            )
            .await
            .expect("update each owner through its view");
        assert_eq!(
            result[0].rows,
            vec![db::Row::new(vec![db::Value::Text("updated".into())])]
        );
        let result = client
            .execute_sql(
                &format!("SELECT {column} FROM {table} WHERE id = $1"),
                Some(&params),
            )
            .await
            .expect("read each owner update");
        assert_eq!(
            result[0].rows,
            vec![db::Row::new(vec![db::Value::Text("updated".into())])]
        );
    }
    assert!(
        shared
            .device()
            .client()
            .row_conflicts("devices", &db::Row::new(vec![db::Value::Uuid(id)]))
            .await
            .expect("PostgreSQL has no CRDT conflicts")
            .is_empty()
    );
    shared
        .client()
        .execute_sql(
            "DELETE FROM sync_selections WHERE id = $1; DELETE FROM namespaces WHERE id = $1; DELETE FROM devices WHERE id = $1; DELETE FROM users WHERE id = $1",
            Some(&params),
        )
        .await
        .expect("clean test rows");
}
