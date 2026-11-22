use of_model::ofdb::MIGRATION;
use ofdb_sql::Client;
use ofdb_sql::QueryError;
use ofdb_sql::SqlTranslator;

/// Runs the unified init migration against the given SQL client.
pub async fn up(client: &Client) -> Result<(), QueryError> {
    let mut transaction = client.transaction().await?;
    let result = async {
        for statement in MIGRATION
            .split(';')
            .map(str::trim)
            .filter(|statement| !statement.is_empty())
        {
            transaction
                .translate_and_execute(statement, &SqlTranslator)
                .await?;
        }
        Ok(())
    }
    .await;
    match result {
        Ok(()) => transaction.commit().await,
        Err(error) => {
            transaction.rollback().await?;
            Err(error)
        }
    }
}

/// Schema validation utilities for the IDP owner.
pub mod validators {
    use ofdb_sql::Client;
    use ofdb_sql::QueryError;
    use ofdb_sql::Row;
    use ofdb_sql::Uuid;
    use ofdb_sql::Value;

    /// Validates that the credential history tables exist and are readable.
    pub async fn validate_credential_history(client: &Client) -> Result<(), QueryError> {
        for sql in [
            "SELECT 1 FROM clients LIMIT 1",
            "SELECT 1 FROM credentials LIMIT 1",
        ] {
            client.execute_sql(sql, None).await?;
        }
        Ok(())
    }

    /// Validates that all IDP security tables exist and are readable.
    pub async fn validate_identity(client: &Client) -> Result<(), QueryError> {
        for table in [
            SecurityTable::User,
            SecurityTable::Application,
            SecurityTable::Client,
            SecurityTable::Credential,
            SecurityTable::Device,
            SecurityTable::Key,
            SecurityTable::IssuerKey,
            SecurityTable::OAuthAuthorizationCode,
            SecurityTable::OAuthRefreshToken,
            SecurityTable::OAuthUserConsent,
            SecurityTable::Permission,
            SecurityTable::Role,
            SecurityTable::RolePermission,
            SecurityTable::UserRole,
        ] {
            client
                .execute_sql(&format!("SELECT 1 FROM {} LIMIT 1", table.name()), None)
                .await?;
        }
        client
            .execute_sql("SELECT policy_kind, application_id, policy_id, incarnation FROM rbac_revocations LIMIT 0", None)
            .await?;
        Ok(())
    }

    /// Security table identifiers for IDP owner validation.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SecurityTable {
        User,
        Application,
        Client,
        Credential,
        Device,
        Key,
        IssuerKey,
        OAuthAuthorizationCode,
        OAuthRefreshToken,
        OAuthUserConsent,
        Permission,
        Role,
        RolePermission,
        UserRole,
    }

    impl SecurityTable {
        #[must_use]
        pub const fn name(self) -> &'static str {
            match self {
                Self::User => "users",
                Self::Application => "applications",
                Self::Client => "clients",
                Self::Credential => "credentials",
                Self::Device => "devices",
                Self::Key => "keys",
                Self::IssuerKey => "issuer_keys",
                Self::OAuthAuthorizationCode => "oauth2_authorization_codes",
                Self::OAuthRefreshToken => "oauth2_refresh_tokens",
                Self::OAuthUserConsent => "oauth2_user_consents",
                Self::Permission => "permissions",
                Self::Role => "roles",
                Self::RolePermission => "role_permissions",
                Self::UserRole => "application_user_roles",
            }
        }

        #[must_use]
        pub const fn is_token_state(self) -> bool {
            matches!(
                self,
                Self::OAuthAuthorizationCode | Self::OAuthRefreshToken | Self::OAuthUserConsent
            )
        }
    }

    /// A reference to a specific row in a security table.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SecurityRow {
        pub table: SecurityTable,
        pub id: Uuid,
    }

    impl SecurityRow {
        #[must_use]
        pub const fn new(table: SecurityTable, id: Uuid) -> Self {
            Self { table, id }
        }

        #[must_use]
        pub fn key(&self) -> Row {
            Row::new(vec![Value::Uuid(self.id)])
        }
    }

    /// Audit record for a conflict resolution.
    #[derive(Debug, Clone)]
    pub struct ResolutionAudit {
        pub id: Uuid,
        pub administrator_id: Uuid,
        pub table: SecurityTable,
        pub row_id: Uuid,
        pub columns: Vec<String>,
        pub created_at: i64,
    }

    /// Checks for conflicts across the given security rows.
    async fn clear(engine: &Client, rows: &[SecurityRow]) -> Result<bool, QueryError> {
        for group in rows.chunk_by(|left, right| left.table == right.table) {
            let table = group[0].table;
            let component = match table {
                SecurityTable::User => "created_at",
                SecurityTable::Credential => "user_id",
                _ => {
                    let keys = group.iter().map(SecurityRow::key).collect::<Vec<_>>();
                    if engine
                        .row_conflicts_batch(table.name(), &keys)
                        .await?
                        .iter()
                        .any(|conflicts| !conflicts.is_empty())
                    {
                        return Ok(false);
                    }
                    continue;
                }
            };
            // Resolve the public UUID and inspect the composite key in one snapshot.
            let mut transaction = engine.transaction().await?;
            let result: Result<bool, QueryError> = async {
                for row in group {
                    let selected = transaction
                        .translate_and_execute(
                            &format!(
                                "SELECT {component} FROM {} WHERE id = CAST('{}' AS UUID)",
                                table.name(),
                                row.id
                            ),
                            &ofdb_sql::SqlTranslator,
                        )
                        .await?;
                    match selected[0].rows.as_slice() {
                        [] => (),
                        [selected] => {
                            let key =
                                Row::new(vec![Value::Uuid(row.id), selected.values[0].clone()]);
                            if !transaction
                                .row_conflicts(table.name(), &key)
                                .await?
                                .is_empty()
                            {
                                return Ok(false);
                            }
                        }
                        _ => return Ok(false),
                    }
                }
                Ok(true)
            }
            .await;
            transaction.rollback().await?;
            if !result? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Checks that the given authorization rows have no conflicts.
    pub async fn allows_authorization(
        engine: &Client,
        rows: &[SecurityRow],
    ) -> Result<bool, QueryError> {
        clear(engine, rows).await
    }

    /// Checks that the given credentials and keys have no conflicts.
    pub async fn allows_authentication(
        engine: &Client,
        credentials: &[Uuid],
        keys: &[Uuid],
    ) -> Result<bool, QueryError> {
        let mut rows = Vec::with_capacity(credentials.len() + keys.len());
        rows.extend(
            credentials
                .iter()
                .copied()
                .map(|id| SecurityRow::new(SecurityTable::Credential, id)),
        );
        rows.extend(
            keys.iter()
                .copied()
                .map(|id| SecurityRow::new(SecurityTable::Key, id)),
        );
        clear(engine, &rows).await
    }

    /// Checks that the given token issuance rows have no conflicts.
    pub async fn allows_token_issuance(
        engine: &Client,
        rows: &[SecurityRow],
    ) -> Result<bool, QueryError> {
        clear(engine, rows).await
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "ofdb")]
    use super::up;

    #[cfg(feature = "ofdb")]
    use ofdb_sql::Database as SqlDatabase;

    #[cfg(feature = "ofdb")]
    #[tokio::test]
    async fn migration_applies_idempotently() {
        let database = SqlDatabase::in_memory();
        let client = database.client();
        up(&client).await.expect("first migration");
        up(&client).await.expect("second migration (idempotent)");
    }
}
