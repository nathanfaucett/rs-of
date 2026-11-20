use ofdb_sql::Database;
use ofdb_sql::SqlTranslator;

pub async fn up(engine: &Database) -> Result<(), ofdb_sql::QueryError> {
    db::ofdb::up(&engine.client()).await?;
    let devices = engine
        .client()
        .translate_and_execute("SELECT * FROM devices LIMIT 0", &SqlTranslator)
        .await?;
    if devices[0].columns.iter().any(|column| {
        matches!(
            column.name.as_str(),
            "enrollment_code_hash" | "enrollment_expires_at" | "pairing_accepting_public_key"
        )
    }) {
        return Err(ofdb_sql::QueryError::new(
            ofdb_sql::QueryErrorKind::Rejected,
            "mixed Device enrollment history is unsupported; initialize a fresh Management database",
        ));
    }
    Ok(())
}

pub async fn validate_device(client: &ofdb_sql::Client) -> Result<(), ofdb_sql::QueryError> {
    for (table, columns) in [
        (
            "devices",
            "id, owner_subject, name, public_key, address, state, approved_at, revoked_at, created_at, updated_at",
        ),
        (
            "local_device_enrollments",
            "device_id, enrollment_code_hash, enrollment_expires_at, pairing_accepting_public_key",
        ),
        (
            "local_device_copy_cleanup",
            "device_id, cleanup_id, completed",
        ),
        (
            "device_selection_policies",
            "device_id, owner_subject, application_id, selected_kind, selected_id, admin_allowed",
        ),
        (
            "device_resource_selections",
            "id, device_id, owner_subject, application_id, selected_kind, selected_id, selected",
        ),
    ] {
        client
            .translate_and_execute(
                &format!("SELECT {columns} FROM {table} LIMIT 0"),
                &SqlTranslator,
            )
            .await?;
    }
    let devices = client
        .translate_and_execute("SELECT * FROM devices LIMIT 0", &SqlTranslator)
        .await?;
    if devices[0].columns.iter().any(|column| {
        matches!(
            column.name.as_str(),
            "enrollment_code_hash" | "enrollment_expires_at" | "pairing_accepting_public_key"
        )
    }) {
        return Err(ofdb_sql::QueryError::new(
            ofdb_sql::QueryErrorKind::Rejected,
            "mixed Device enrollment history is unsupported",
        ));
    }
    let key = ofdb_sql::Row::new(vec![ofdb_sql::Value::Uuid(ofdb_sql::Uuid::nil())]);
    client
        .row_conflicts_batch("devices", core::slice::from_ref(&key))
        .await?;
    let policy_key = ofdb_sql::Row::new(vec![ofdb_sql::Value::Text(
        ofdb_sql::Uuid::nil().to_string(),
    )]);
    client
        .row_conflict_values("device_selection_policies", &policy_key)
        .await?;
    let mut transaction = client.transaction().await?;
    transaction.row_conflicts("devices", &key).await?;
    transaction
        .row_conflict_values("device_selection_policies", &policy_key)
        .await?;
    transaction.rollback().await
}

#[cfg(test)]
mod tests {

    use crate::ofdb::schema::up;

    #[tokio::test]
    async fn device_owner_does_not_migrate_identity_policy() {
        let engine = ofdb_sql::Database::in_memory();
        crate::ofdb::schema::up(&engine)
            .await
            .expect("Device migration");
        crate::ofdb::schema::up(&engine)
            .await
            .expect("idempotent Device migration");
        for table in [
            "devices",
            "device_selection_policies",
            "local_device_enrollments",
        ] {
            engine
                .client()
                .translate_and_execute(
                    &format!("SELECT * FROM {table} LIMIT 0"),
                    &ofdb_sql::SqlTranslator,
                )
                .await
                .expect("Device table");
        }
        for table in ["roles", "permissions", "users", "clients"] {
            assert!(
                engine
                    .client()
                    .translate_and_execute(
                        &format!("SELECT * FROM {table} LIMIT 0"),
                        &ofdb_sql::SqlTranslator
                    )
                    .await
                    .is_err(),
                "identity policy must stay separate"
            );
        }
    }

    #[tokio::test]
    async fn rejects_mixed_public_device_enrollment_history_without_repair() {
        let engine = ofdb_sql::Database::in_memory();
        engine.client().translate_and_execute("CREATE TABLE devices (id UUID PRIMARY KEY, owner_subject TEXT NOT NULL, name TEXT, public_key TEXT, address TEXT, state INTEGER, approved_at INTEGER, revoked_at INTEGER, created_at INTEGER, updated_at INTEGER, enrollment_code_hash BLOB)", &ofdb_sql::SqlTranslator).await.expect("unsupported retained public layout");
        assert!(crate::ofdb::schema::up(&engine).await.is_err());
        let result = engine
            .client()
            .translate_and_execute(
                "SELECT enrollment_code_hash FROM devices LIMIT 0",
                &ofdb_sql::SqlTranslator,
            )
            .await;
        assert!(
            result.is_ok(),
            "failed open must not mutate away retained history"
        );
    }

    #[tokio::test]
    async fn initializes_device_tables_without_identity_tables() {
        let engine = ofdb_sql::Database::in_memory();

        up(&engine).await.expect("initialize management schema");

        for table in ["device_selection_policies", "devices"] {
            engine
                .client()
                .translate_and_execute(&format!("SELECT * FROM {table}"), &ofdb_sql::SqlTranslator)
                .await
                .expect("management-owned table exists");
        }
        for table in [
            "roles",
            "permissions",
            "role_permissions",
            "application_user_roles",
            "users",
            "applications",
            "clients",
            "keys",
        ] {
            assert!(
                engine
                    .client()
                    .translate_and_execute(
                        &format!("SELECT * FROM {table}"),
                        &ofdb_sql::SqlTranslator
                    )
                    .await
                    .is_err()
            );
        }
    }
}
