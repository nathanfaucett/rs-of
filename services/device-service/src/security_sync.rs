/// Device policy records belong to the Device authority, never the identity store. Each apply
/// still require authenticated peer provenance and locally evaluated permission.
pub const PUBLIC_DEVICE_TABLES: &[&str] = &[
    "devices",
    "device_selection_policies",
    "device_resource_selections",
    "device_selection_denials",
];

#[must_use]
pub fn public_device_columns(table: &str) -> Option<&'static [&'static str]> {
    Some(match table {
        "devices" => &[
            "id",
            "owner_subject",
            "name",
            "public_key",
            "address",
            "state",
            "approved_at",
            "revoked_at",
            "created_at",
            "updated_at",
        ],
        "device_selection_policies" => &[
            "device_id",
            "owner_subject",
            "application_id",
            "selected_kind",
            "selected_id",
            "admin_allowed",
            "device_key",
        ],
        "device_resource_selections" => &[
            "id",
            "device_id",
            "owner_subject",
            "application_id",
            "selected_kind",
            "selected_id",
            "selected",
        ],
        "device_selection_denials" => &["id", "selection_id"],
        _ => return None,
    })
}

#[must_use]
pub fn permits_device_table(table: &str) -> bool {
    PUBLIC_DEVICE_TABLES.contains(&table)
}

#[cfg(test)]
mod tests {
    use crate::security_sync::permits_device_table;

    #[cfg(feature = "ofdb")]
    #[tokio::test]
    async fn public_device_columns_match_only_the_owned_schema() {
        let database = ofdb_sql::Database::in_memory();
        crate::ofdb::schema::up(&database)
            .await
            .expect("declare policy schema");
        for table in crate::security_sync::PUBLIC_DEVICE_TABLES {
            let expected = crate::security_sync::public_device_columns(table)
                .expect("explicitly reviewed policy table");
            let result = database
                .client()
                .translate_and_execute(
                    &alloc::format!("SELECT * FROM {table} LIMIT 0"),
                    &ofdb_sql::SqlTranslator,
                )
                .await
                .expect("public schema columns");
            assert_eq!(result[0].columns.len(), expected.len());
            assert!(
                result[0]
                    .columns
                    .iter()
                    .all(|column| expected.contains(&column.name.as_str()))
            );
        }
        assert_eq!(
            crate::security_sync::public_device_columns("local_client_secret_verifiers"),
            None
        );
        assert!(crate::security_sync::public_device_columns("devices").is_some());
        assert_eq!(
            crate::security_sync::public_device_columns("local_device_enrollments"),
            None
        );
        assert_eq!(crate::security_sync::public_device_columns("unknown"), None);
    }

    #[cfg(feature = "ofdb")]
    #[test]
    fn device_owner_validates_denial_identity_and_binding() {
        use crate::security_policy::SecurityHistory;
        use ofdb_sql::history::{
            SyncCandidateView, SyncRowView, SyncTableView, SyncValidationDirection,
        };
        use ofdb_sql::{Row, Uuid, Value};

        let event = Uuid::now_v7();
        let selection = Uuid::new_v5(&Uuid::NAMESPACE_URL, b"selection");
        let owner = SecurityHistory::Devices;
        let table = "device_selection_denials";
        assert!(
            owner
                .validate_value(table, "selection_id", &Value::Uuid(event), "fixture")
                .is_err()
        );
        assert!(
            owner
                .validate_value(table, "selection_id", &Value::Uuid(selection), "fixture")
                .is_ok()
        );
        let mut view = SyncCandidateView {
            direction: SyncValidationDirection::Incoming,
            candidates: vec![],
            tables: vec![SyncTableView {
                name: table.into(),
                columns: vec!["id".into(), "selection_id".into()],
                rows: vec![SyncRowView {
                    id: event,
                    row: Row::new(vec![Value::Uuid(event), Value::Uuid(selection)]),
                    deleted: false,
                    conflicts: vec![],
                }],
            }],
        };
        assert!(owner.validate_candidate(&view, &[]).is_ok());
        view.tables[0].rows[0].row.values[1] = Value::Null;
        assert!(owner.validate_candidate(&view, &[]).is_err());
        view.tables[0].rows[0].row.values[1] = Value::Uuid(selection);
        view.tables[0].rows[0].id = Uuid::now_v7();
        assert!(owner.validate_candidate(&view, &[]).is_err());
    }

    #[test]
    fn policy_sync_is_context_local_and_explicitly_allowlisted() {
        assert!(permits_device_table("devices"));
        assert!(!permits_device_table("role_permissions"));
        assert!(permits_device_table("device_selection_policies"));
        assert!(permits_device_table("device_resource_selections"));
        for table in [
            "roles",
            "permissions",
            "application_user_roles",
            "users",
            "clients",
            "keys",
            "oauth2_refresh_tokens",
            "local_client_secret_verifiers",
            "local_user_password_verifiers",
            "local_device_enrollments",
            "unknown",
        ] {
            assert!(!permits_device_table(table));
        }
    }
}
