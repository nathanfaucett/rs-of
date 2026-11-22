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

    /// Validates that all IDP security tables exist and are readable.
    pub async fn validate_identity(client: &Client) -> Result<(), QueryError> {
        for table in [
            SecurityTable::User,
            SecurityTable::Application,
            SecurityTable::Client,
            SecurityTable::Device,
            SecurityTable::Key,
            SecurityTable::OAuthAuthorizationCode,
            SecurityTable::OAuthRefreshToken,
            SecurityTable::OAuthUserConsent,
            SecurityTable::Permission,
            SecurityTable::Role,
            SecurityTable::RolePermission,
            SecurityTable::UserRole,
            SecurityTable::ReplicaMember,
            SecurityTable::ReplicaSigner,
        ] {
            client
                .execute_sql(&format!("SELECT 1 FROM {} LIMIT 1", table.name()), None)
                .await?;
        }
        client
            .execute_sql(
                "SELECT name, user_id, actions FROM administrator_provisioning LIMIT 0",
                None,
            )
            .await?;
        Ok(())
    }

    /// Security table identifiers for IDP owner validation.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SecurityTable {
        User,
        Application,
        Client,
        Device,
        Key,
        OAuthAuthorizationCode,
        OAuthRefreshToken,
        OAuthUserConsent,
        Permission,
        Role,
        RolePermission,
        UserRole,
        ReplicaMember,
        ReplicaSigner,
    }

    impl SecurityTable {
        #[must_use]
        pub const fn name(self) -> &'static str {
            match self {
                Self::User => "users",
                Self::Application => "applications",
                Self::Client => "clients",
                Self::Device => "devices",
                Self::Key => "keys",
                Self::OAuthAuthorizationCode => "oauth2_authorization_codes",
                Self::OAuthRefreshToken => "oauth2_refresh_tokens",
                Self::OAuthUserConsent => "oauth2_user_consents",
                Self::Permission => "permissions",
                Self::Role => "roles",
                Self::RolePermission => "role_permissions",
                Self::UserRole => "application_user_roles",
                Self::ReplicaMember => "idp_replica_members",
                Self::ReplicaSigner => "idp_replica_signers",
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

    /// Checks for conflicts across the given security rows.
    async fn clear(engine: &Client, rows: &[SecurityRow]) -> Result<bool, QueryError> {
        for group in rows.chunk_by(|left, right| left.table == right.table) {
            let table = group[0].table;
            let component = match table {
                SecurityTable::User => "created_at",
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

    /// Checks that the given token issuance rows have no conflicts.
    pub async fn allows_token_issuance(
        engine: &Client,
        rows: &[SecurityRow],
    ) -> Result<bool, QueryError> {
        clear(engine, rows).await
    }

    /// Inspect candidate (signer row ID, member row ID) pairs without combining their decisions.
    /// Clearance proves only conflict absence; callers must validate the current trust records.
    pub async fn allows_signer_trust_batch(
        engine: &Client,
        pairs: &[(Uuid, Uuid)],
    ) -> Result<Vec<bool>, QueryError> {
        if pairs.is_empty() {
            return Ok(Vec::new());
        }
        let signers = pairs
            .iter()
            .map(|(signer, _)| Row::new(vec![Value::Uuid(*signer)]))
            .collect::<Vec<_>>();
        let members = pairs
            .iter()
            .map(|(_, member)| Row::new(vec![Value::Uuid(*member)]))
            .collect::<Vec<_>>();
        let signer_conflicts = engine
            .row_conflicts_batch("idp_replica_signers", &signers)
            .await?;
        let member_conflicts = engine
            .row_conflicts_batch("idp_replica_members", &members)
            .await?;
        Ok(signer_conflicts
            .into_iter()
            .zip(member_conflicts)
            .map(|(s, m)| s.is_empty() && m.is_empty())
            .collect())
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
