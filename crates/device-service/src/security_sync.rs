/// Device policy records belong to the Device authority, never the identity store. Each apply
/// still require authenticated peer provenance and locally evaluated permission.
pub const PUBLIC_DEVICE_TABLES: &[&str] = &[
    "devices",
    "device_selection_policies",
    "device_resource_selections",
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
        _ => return None,
    })
}

#[must_use]
pub fn permits_device_table(table: &str) -> bool {
    PUBLIC_DEVICE_TABLES.contains(&table)
}

#[cfg(test)]
mod tests {
    use super::permits_device_table;

    #[cfg(feature = "replica")]
    #[tokio::test]
    async fn public_device_columns_match_only_the_owned_schema() {
        let database = db::Database::in_memory();
        crate::replica::up(&database)
            .await
            .expect("declare policy schema");
        for table in super::PUBLIC_DEVICE_TABLES {
            let expected =
                super::public_device_columns(table).expect("explicitly reviewed policy table");
            let result = database
                .client()
                .translate_and_execute(
                    &alloc::format!("SELECT * FROM {table} LIMIT 0"),
                    &db::SqlTranslator,
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
            super::public_device_columns("local_client_credentials"),
            None
        );
        assert!(super::public_device_columns("devices").is_some());
        assert_eq!(
            super::public_device_columns("local_device_enrollments"),
            None
        );
        assert_eq!(super::public_device_columns("unknown"), None);
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
            "credentials",
            "clients",
            "keys",
            "oauth2_refresh_tokens",
            "local_client_credentials",
            "local_credential_secrets",
            "local_device_enrollments",
            "unknown",
        ] {
            assert!(!permits_device_table(table));
        }
    }
}
