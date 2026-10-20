use include_dir::{Dir, include_dir};

use db::{Database, Row, SqlTranslator, Uuid, Value};

static MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/replica_migrations");

pub async fn up(engine: &Database) -> Result<(), db::QueryError> {
    for file in MIGRATIONS.files() {
        for statement in file
            .contents_utf8()
            .expect("replica migration must be UTF-8")
            .split(';')
            .map(str::trim)
            .filter(|statement| !statement.is_empty())
        {
            engine
                .client()
                .translate_and_execute(statement, &SqlTranslator)
                .await?;
        }
    }
    for table in ["clients", "credentials"] {
        let results = engine
            .client()
            .translate_and_execute(&format!("SELECT * FROM {table}"), &SqlTranslator)
            .await?;
        if results[0].columns.iter().any(|column| {
            matches!(
                column.name.as_str(),
                "client_secret_hash"
                    | "secret_hash"
                    | "enrollment_code_hash"
                    | "enrollment_expires_at"
                    | "pairing_accepting_public_key"
            )
        }) {
            return Err(db::QueryError::new(
                db::QueryErrorKind::Rejected,
                "mixed credential history is unsupported; initialize a fresh identity database",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecurityTable {
    User,
    Application,
    Client,
    Credential,
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
    const fn name(self) -> &'static str {
        match self {
            Self::User => "users",
            Self::Application => "applications",
            Self::Client => "clients",
            Self::Credential => "credentials",
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

    const fn is_token_state(self) -> bool {
        matches!(self, Self::OAuthAuthorizationCode | Self::OAuthRefreshToken)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecurityRow {
    pub table: SecurityTable,
    pub id: Uuid,
}

impl SecurityRow {
    #[must_use]
    pub const fn new(table: SecurityTable, id: Uuid) -> Self {
        Self { table, id }
    }

    fn key(self) -> Row {
        Row::new(vec![Value::Uuid(self.id)])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionAudit {
    pub id: Uuid,
    pub administrator_id: Uuid,
    pub table: SecurityTable,
    pub row_id: Uuid,
    pub columns: Vec<String>,
    pub created_at: i64,
}

pub async fn allows_authentication(
    engine: &Database,
    credentials: &[Uuid],
    keys: &[Uuid],
) -> Result<bool, db::QueryError> {
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

pub async fn allows_authorization(
    engine: &Database,
    rows: &[SecurityRow],
) -> Result<bool, db::QueryError> {
    clear(engine, rows).await
}

/// Inspect candidate (signer row ID, member row ID) pairs without combining their decisions.
/// Clearance proves only conflict absence; callers must validate the current trust records.
pub async fn allows_signer_trust_batch(
    engine: &Database,
    pairs: &[(Uuid, Uuid)],
) -> Result<Vec<bool>, db::QueryError> {
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
        .client()
        .row_conflicts_batch(SecurityTable::ReplicaSigner.name(), &signers)
        .await?;
    let member_conflicts = engine
        .client()
        .row_conflicts_batch(SecurityTable::ReplicaMember.name(), &members)
        .await?;
    Ok(signer_conflicts
        .iter()
        .zip(&member_conflicts)
        .map(|(signer, member)| signer.is_empty() && member.is_empty())
        .collect())
}

pub async fn allows_token_issuance(
    engine: &Database,
    rows: &[SecurityRow],
) -> Result<bool, db::QueryError> {
    clear(engine, rows).await
}

pub async fn resolve(
    engine: &Database,
    administrator_id: Uuid,
    row: SecurityRow,
    values: Vec<(String, Value)>,
    created_at: i64,
) -> Result<Option<ResolutionAudit>, db::QueryError> {
    let conflicts = engine
        .client()
        .row_conflicts(row.table.name(), &row.key())
        .await?;
    let unsafe_lifecycle = ["revoked_at", "consumed_at"].iter().any(|column| {
        let monotonic = *column == "revoked_at" || row.table.is_token_state();
        monotonic
            && (values
                .iter()
                .any(|(field, value)| field == column && matches!(value, Value::Null))
                || (conflicts.iter().any(|field| field == column)
                    && !values.iter().any(|(field, value)| {
                        field == column && matches!(value, Value::Integer(_))
                    })))
    });
    if administrator_id.is_nil() || conflicts.is_empty() || unsafe_lifecycle {
        return Ok(None);
    }

    let audit = ResolutionAudit {
        id: Uuid::now_v7(),
        administrator_id,
        table: row.table,
        row_id: row.id,
        columns: conflicts,
        created_at,
    };
    engine
        .client()
        .resolve_row(row.table.name(), &row.key(), values)
        .await?;
    engine
        .client()
        .translate_and_execute(&resolution_insert(&audit), &SqlTranslator)
        .await?;
    Ok(Some(audit))
}

async fn clear(engine: &Database, rows: &[SecurityRow]) -> Result<bool, db::QueryError> {
    for group in rows.chunk_by(|left, right| left.table == right.table) {
        let keys = group.iter().map(|row| row.key()).collect::<Vec<_>>();
        if engine
            .client()
            .row_conflicts_batch(group[0].table.name(), &keys)
            .await?
            .iter()
            .any(|conflicts| !conflicts.is_empty())
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn resolution_insert(audit: &ResolutionAudit) -> String {
    format!(
        "INSERT INTO idp_conflict_resolutions (id, administrator_id, table_name, row_id, columns, created_at) VALUES (CAST('{}' AS UUID), CAST('{}' AS UUID), '{}', CAST('{}' AS UUID), '{}', {})",
        audit.id,
        audit.administrator_id,
        audit.table.name(),
        audit.row_id,
        audit.columns.join(","),
        audit.created_at,
    )
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::{
        SecurityRow, SecurityTable, allows_authentication, allows_authorization,
        allows_token_issuance, resolve, up,
    };

    const ADMIN: u128 = 100;
    const DEVICE: u128 = 1;
    const CLIENT: u128 = 2;
    const ROLE: u128 = 3;
    const CREDENTIAL: u128 = 4;
    const KEY: u128 = 5;
    const CODE: u128 = 6;
    const REFRESH: u128 = 7;

    #[test]
    fn mixed_legacy_client_schema_is_rejected_not_silently_migrated() {
        block_on(async {
            let database = db::Database::in_memory();
            for file in super::MIGRATIONS.files() {
                let legacy = file.contents_utf8().expect("schema UTF-8").replace(
                    "    client_id TEXT,",
                    "    client_id TEXT,\n    client_secret_hash TEXT,",
                );
                for statement in legacy
                    .split(';')
                    .map(str::trim)
                    .filter(|statement| !statement.is_empty())
                {
                    database
                        .client()
                        .translate_and_execute(statement, &db::SqlTranslator)
                        .await
                        .expect("create legacy mixed schema");
                }
            }
            let error = up(&database)
                .await
                .expect_err("old mixed-history store must not qualify as separated");
            assert!(error.message.contains("fresh"));
        });
    }

    #[test]
    fn identity_schema_initializes_idempotently_and_rejects_duplicate_device_bindings() {
        block_on(async {
            let engine = db::Database::in_memory();
            up(&engine).await.unwrap();
            up(&engine).await.unwrap();

            let clients_schema = engine
                .client()
                .translate_and_execute("SELECT * FROM clients", &db::SqlTranslator)
                .await
                .expect("query clients")
                .remove(0);
            assert!(
                clients_schema
                    .columns
                    .iter()
                    .any(|column| column.name == "allowed_audiences")
            );

            for table in [
                "users",
                "user_emails",
                "user_phone_numbers",
                "credentials",
                "keys",
                "applications",
                "clients",
                "devices",
                "roles",
                "permissions",
                "role_permissions",
                "application_user_roles",
                "oauth2_authorization_codes",
                "oauth2_user_consents",
                "oauth2_refresh_tokens",
                "idp_conflict_resolutions",
                "idp_replica_members",
                "idp_replica_signers",
            ] {
                let result = engine
                    .client()
                    .translate_and_execute(&format!("SELECT * FROM {table}"), &db::SqlTranslator)
                    .await
                    .expect("query identity table")
                    .remove(0);
                assert_eq!(result.columns[0].name, "id");
            }

            engine.client()
                .translate_and_execute(
                    "INSERT INTO devices (id, public_key) VALUES (CAST('00000000-0000-7000-8000-000000000001' AS UUID), 'key')",
                    &db::SqlTranslator,
                )
                .await
                .unwrap();
            assert!(engine.client()
                .translate_and_execute(
                    "INSERT INTO devices (id, public_key) VALUES (CAST('00000000-0000-7000-8000-000000000002' AS UUID), 'key')",
                    &db::SqlTranslator,
                )
                .await
                .is_err());

            engine.client()
                .translate_and_execute(
                    "INSERT INTO idp_replica_members (id, installation_id, member_id, endpoint_id, issuer, approved_at) VALUES (CAST('00000000-0000-7000-8000-000000000010' AS UUID), 'installation-a', CAST('00000000-0000-7000-8000-000000000011' AS UUID), 'endpoint-a', 'https://issuer.example', 1)",
                    &db::SqlTranslator,
                )
                .await
                .expect("insert first replica membership");
            assert!(engine.client()
                .translate_and_execute(
                    "INSERT INTO idp_replica_members (id, installation_id, member_id, endpoint_id, issuer, approved_at) VALUES (CAST('00000000-0000-7000-8000-000000000012' AS UUID), 'installation-a', CAST('00000000-0000-7000-8000-000000000011' AS UUID), 'endpoint-b', 'https://issuer.example', 1)",
                    &db::SqlTranslator,
                )
                .await
                .is_err());
            assert!(engine.client()
                .translate_and_execute(
                    "INSERT INTO idp_replica_members (id, installation_id, member_id, endpoint_id, issuer, approved_at) VALUES (CAST('00000000-0000-7000-8000-000000000013' AS UUID), 'installation-a', CAST('00000000-0000-7000-8000-000000000014' AS UUID), 'endpoint-a', 'https://issuer.example', 1)",
                    &db::SqlTranslator,
                )
                .await
                .is_err());
        });
    }

    #[test]
    fn reconnect_conflict_resolution_cannot_resurrect_revoked_refresh_grants() {
        block_on(async {
            let source = db::Database::in_memory();
            let destination = db::Database::in_memory();
            up(&source).await.expect("initialize grant source");
            seed(&source).await;
            synchronize(&source, &destination).await;
            update(&source, "oauth2_refresh_tokens", REFRESH, "revoked_at", "1").await;
            update(
                &destination,
                "oauth2_refresh_tokens",
                REFRESH,
                "revoked_at",
                "2",
            )
            .await;
            synchronize(&source, &destination).await;
            synchronize(&destination, &source).await;
            let grant = SecurityRow::new(SecurityTable::OAuthRefreshToken, uuid(REFRESH));
            assert!(
                !allows_token_issuance(&source, &[grant])
                    .await
                    .expect("conflicting grants deny")
            );
            assert!(
                resolve(
                    &source,
                    uuid(ADMIN),
                    grant,
                    vec![("revoked_at".into(), db::Value::Null)],
                    3
                )
                .await
                .expect("reject resurrection")
                .is_none()
            );
            assert!(
                resolve(
                    &source,
                    uuid(ADMIN),
                    grant,
                    vec![("revoked_at".into(), db::Value::Integer(1))],
                    3
                )
                .await
                .expect("retain revocation")
                .is_some()
            );
        });
    }

    #[test]
    fn signer_trust_batch_preserves_pair_local_conflicts_and_refreshes() {
        block_on(async {
            for table in ["idp_replica_members", "idp_replica_signers"] {
                let source = db::Database::in_memory();
                let destination = db::Database::in_memory();
                up(&source).await.expect("initialize trust schema");
                for id in [101, 102] {
                    let row_id = uuid(id);
                    source.client().translate_and_execute(
                        &format!(
                            "INSERT INTO idp_replica_members (id, installation_id, member_id, endpoint_id, issuer, approved_at) VALUES (CAST('{row_id}' AS UUID), 'issuer', CAST('{row_id}' AS UUID), 'endpoint-{id}', 'issuer', 1); INSERT INTO idp_replica_signers (id, member_id, active_member_id, key_id, issuer, public_jwk, approved_at) VALUES (CAST('{row_id}' AS UUID), CAST('{row_id}' AS UUID), CAST('{row_id}' AS UUID), CAST('{row_id}' AS UUID), 'issuer', '{{}}', 1)"
                        ),
                        &db::SqlTranslator,
                    ).await.expect("seed independent trust pairs");
                }
                let pairs = [
                    (uuid(101), uuid(101)),
                    (uuid(102), uuid(102)),
                    (uuid(101), uuid(101)),
                ];
                assert!(
                    super::allows_signer_trust_batch(&source, &[])
                        .await
                        .expect("empty batch")
                        .is_empty()
                );
                assert_eq!(
                    super::allows_signer_trust_batch(&source, &pairs)
                        .await
                        .expect("clear pairs"),
                    vec![true, true, true]
                );
                synchronize(&source, &destination).await;
                update(&source, table, 101, "approved_at", "2").await;
                update(&destination, table, 101, "approved_at", "3").await;
                synchronize(&destination, &source).await;
                assert_eq!(
                    super::allows_signer_trust_batch(&source, &pairs)
                        .await
                        .expect("pair-local conflicts"),
                    vec![false, true, false],
                    "{table}"
                );
                let table = if table == "idp_replica_members" {
                    SecurityTable::ReplicaMember
                } else {
                    SecurityTable::ReplicaSigner
                };
                resolve(
                    &source,
                    uuid(ADMIN),
                    SecurityRow::new(table, uuid(101)),
                    vec![("approved_at".into(), db::Value::Integer(4))],
                    4,
                )
                .await
                .expect("resolve trust conflict")
                .expect("conflict was present");
                assert_eq!(
                    super::allows_signer_trust_batch(&source, &pairs)
                        .await
                        .expect("fresh evidence"),
                    vec![true, true, true]
                );
            }
        });
    }

    #[test]
    fn authorization_checks_every_selected_row_and_refreshes_after_resolution() {
        block_on(async {
            let source = db::Database::in_memory();
            let destination = db::Database::in_memory();
            up(&source).await.expect("initialize identity schema");
            seed(&source).await;
            let second_role = uuid(101);
            source
                .client()
                .translate_and_execute(
                    &format!(
                        "INSERT INTO roles (id, name) VALUES (CAST('{second_role}' AS UUID), 'second')"
                    ),
                    &db::SqlTranslator,
                )
                .await
                .expect("insert second role");
            synchronize(&source, &destination).await;
            let roles = [
                SecurityRow::new(SecurityTable::Role, uuid(ROLE)),
                SecurityRow::new(SecurityTable::Role, second_role),
            ];
            let mixed = [
                roles[0],
                SecurityRow::new(SecurityTable::Client, uuid(CLIENT)),
                roles[1],
            ];
            for selected in [&roles[..], &mixed[..]] {
                assert!(
                    allows_authorization(&source, selected)
                        .await
                        .expect("inspect clear authorization state")
                );
            }

            update(&source, "roles", 101, "name", "'source'").await;
            update(&destination, "roles", 101, "name", "'destination'").await;
            synchronize(&source, &destination).await;
            synchronize(&destination, &source).await;
            for selected in [&roles[..], &mixed[..]] {
                assert!(
                    !allows_authorization(&source, selected)
                        .await
                        .expect("inspect conflicted authorization state")
                );
            }

            assert!(
                resolve(
                    &source,
                    uuid(ADMIN),
                    roles[1],
                    vec![("name".to_owned(), db::Value::Text("resolved".to_owned()))],
                    3,
                )
                .await
                .expect("resolve second role")
                .is_some()
            );
            for selected in [&roles[..], &mixed[..]] {
                assert!(
                    allows_authorization(&source, selected)
                        .await
                        .expect("inspect freshly resolved authorization state")
                );
            }
        });
    }

    #[test]
    fn security_decisions_fail_closed_until_an_administrator_resolves_the_selected_state() {
        block_on(async {
            let source = db::Database::in_memory();
            let destination = db::Database::in_memory();
            up(&source).await.unwrap();
            seed(&source).await;
            synchronize(&source, &destination).await;

            for (table, id, column) in [
                ("devices", DEVICE, "state"),
                ("clients", CLIENT, "client_name"),
                ("roles", ROLE, "name"),
                ("credentials", CREDENTIAL, "active"),
                ("keys", KEY, "name"),
                ("oauth2_authorization_codes", CODE, "consumed_at"),
                ("oauth2_refresh_tokens", REFRESH, "consumed_at"),
            ] {
                update(&source, table, id, column, "1").await;
                update(&destination, table, id, column, "2").await;
            }
            synchronize(&source, &destination).await;
            synchronize(&destination, &source).await;

            assert!(
                !allows_authorization(
                    &source,
                    &[SecurityRow::new(SecurityTable::Device, uuid(DEVICE))],
                )
                .await
                .unwrap()
            );
            assert!(
                !allows_token_issuance(
                    &source,
                    &[SecurityRow::new(SecurityTable::Client, uuid(CLIENT))],
                )
                .await
                .unwrap()
            );
            assert!(
                !allows_authorization(
                    &source,
                    &[SecurityRow::new(SecurityTable::Role, uuid(ROLE))],
                )
                .await
                .unwrap()
            );
            assert!(
                !allows_authentication(&source, &[uuid(CREDENTIAL)], &[uuid(KEY)])
                    .await
                    .unwrap()
            );
            assert!(
                !allows_token_issuance(
                    &source,
                    &[
                        SecurityRow::new(SecurityTable::OAuthAuthorizationCode, uuid(CODE)),
                        SecurityRow::new(SecurityTable::OAuthRefreshToken, uuid(REFRESH)),
                    ],
                )
                .await
                .unwrap()
            );

            let code = SecurityRow::new(SecurityTable::OAuthAuthorizationCode, uuid(CODE));
            assert!(
                resolve(
                    &source,
                    uuid(ADMIN),
                    code,
                    vec![("consumed_at".to_owned(), db::Value::Null)],
                    3,
                )
                .await
                .unwrap()
                .is_none()
            );
            let audit = resolve(
                &source,
                uuid(ADMIN),
                code,
                vec![("consumed_at".to_owned(), db::Value::Integer(1))],
                3,
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(audit.row_id, uuid(CODE));
            assert_eq!(
                source
                    .client()
                    .translate_and_execute(
                        "SELECT id FROM idp_conflict_resolutions",
                        &db::SqlTranslator,
                    )
                    .await
                    .unwrap()[0]
                    .rows
                    .len(),
                1
            );
            assert!(allows_token_issuance(&source, &[code]).await.unwrap());
        });
    }

    async fn seed(engine: &db::Database) {
        for (table, id, column, value) in [
            ("devices", DEVICE, "state", "0"),
            ("clients", CLIENT, "client_name", "'client'"),
            ("roles", ROLE, "name", "'role'"),
            ("credentials", CREDENTIAL, "active", "0"),
            ("keys", KEY, "name", "'key'"),
            ("oauth2_authorization_codes", CODE, "consumed_at", "NULL"),
            ("oauth2_refresh_tokens", REFRESH, "consumed_at", "NULL"),
        ] {
            engine
                .client()
                .translate_and_execute(
                    &format!(
                        "INSERT INTO {table} (id, {column}) VALUES (CAST('{}' AS UUID), {value})",
                        uuid(id)
                    ),
                    &db::SqlTranslator,
                )
                .await
                .unwrap();
        }
    }

    struct Channel {
        sender: futures::channel::mpsc::UnboundedSender<Vec<u8>>,
        receiver: futures::channel::mpsc::UnboundedReceiver<Vec<u8>>,
    }

    impl ofdb_sql::SyncTransport for Channel {
        type Error = &'static str;
        async fn send(&mut self, frame: Vec<u8>) -> Result<(), Self::Error> {
            self.sender.unbounded_send(frame).map_err(|_| "closed")
        }
        async fn receive(&mut self) -> Result<Vec<u8>, Self::Error> {
            use futures::StreamExt;
            self.receiver.next().await.ok_or("closed")
        }
    }

    async fn synchronize(source: &db::Database, destination: &db::Database) {
        let (a, ar) = futures::channel::mpsc::unbounded();
        let (b, br) = futures::channel::mpsc::unbounded();
        let mut left = Channel {
            sender: a,
            receiver: br,
        };
        let mut right = Channel {
            sender: b,
            receiver: ar,
        };
        let config = ofdb_sql::SessionConfig::default();
        let (sent, received) = futures::future::join(
            source.synchronize(&mut left, &config, ofdb_sql::SyncRole::Initiator),
            destination.synchronize(&mut right, &config, ofdb_sql::SyncRole::Responder),
        )
        .await;
        sent.expect("sync trusted domain fixture");
        received.expect("receive trusted domain fixture");
    }

    async fn update(engine: &db::Database, table: &str, id: u128, column: &str, value: &str) {
        let result = engine
            .client()
            .translate_and_execute(
                &format!(
                    "UPDATE {table} SET {column} = {value} WHERE id = CAST('{}' AS UUID)",
                    uuid(id)
                ),
                &db::SqlTranslator,
            )
            .await;
        assert!(result.is_ok(), "{table}: {result:?}");
    }

    fn uuid(value: u128) -> db::Uuid {
        db::Uuid::from_u128(0x00000000000070008000000000000000 | value)
    }
}
