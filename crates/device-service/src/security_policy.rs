use std::{io, sync::Arc};

use db::{Database, Value};
use idp_model::{
    contract::{
        IdentityAction, IdentityResource, PermissionEvaluationRequest, PermissionSubject,
        PermissionTarget,
    },
    model::Id,
};
use idp_service::{
    replica::LocalIdentity,
    security_sync::{PUBLIC_IDENTITY_POLICY_TABLES, public_security_tables},
};
use management_service::LocalPermissions;
use ofdb_sql::SyncCandidateView;
use serde::{Deserialize, Serialize};

use crate::{
    DeviceRepo,
    replica::DbDeviceRepo,
    security_candidates::{self, JoinCandidate, PeerBinding},
    security_sync::{PUBLIC_DEVICE_TABLES, public_device_columns},
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityContext {
    Idp,
    Device,
}

impl SecurityContext {
    pub fn histories(self) -> &'static [SecurityHistory] {
        match self {
            Self::Idp => &[SecurityHistory::Idp],
            Self::Device => &[SecurityHistory::IdentityPolicy, SecurityHistory::Devices],
        }
    }

    fn permission(self) -> (IdentityAction, IdentityResource) {
        match self {
            Self::Idp => (
                IdentityAction::IdpSecuritySync,
                IdentityResource::IdpSecurity,
            ),
            Self::Device => (
                IdentityAction::ManagementSecuritySync,
                IdentityResource::ManagementSecurity,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecurityHistory {
    Idp,
    IdentityPolicy,
    Devices,
}

impl SecurityHistory {
    pub fn name(self) -> &'static str {
        match self {
            Self::Idp => "idp",
            Self::IdentityPolicy => "policy-identity",
            Self::Devices => "policy-devices",
        }
    }

    pub fn tables(self) -> Vec<&'static str> {
        match self {
            Self::Idp => public_security_tables()
                .into_iter()
                .map(|table| table.name())
                .filter(|name| {
                    !PUBLIC_IDENTITY_POLICY_TABLES
                        .iter()
                        .any(|table| table.name() == *name)
                })
                .collect(),
            Self::IdentityPolicy => PUBLIC_IDENTITY_POLICY_TABLES
                .iter()
                .map(|table| table.name())
                .collect(),
            Self::Devices => PUBLIC_DEVICE_TABLES.to_vec(),
        }
    }

    pub fn validate_value(
        self,
        table: &str,
        column: &str,
        value: &Value,
        installation: &str,
    ) -> Result<(), String> {
        let permitted = self.tables().contains(&table)
            && match self {
                Self::Idp | Self::IdentityPolicy => {
                    public_security_tables()
                        .into_iter()
                        .find(|public| public.name() == table)
                        .is_some_and(|public| public.permits_historical_value(column, value))
                        && security_candidates::permits_installation(
                            table,
                            column,
                            value,
                            installation,
                        )
                }
                Self::Devices => public_device_columns(table).is_some_and(|columns| {
                    columns.contains(&column)
                        && (table != "devices"
                            || security_candidates::permits_device_value(column, value))
                }),
            };
        if permitted {
            Ok(())
        } else {
            Err(format!(
                "invalid public security historical value: {table}.{column}"
            ))
        }
    }

    pub fn validate_candidate(
        self,
        view: &SyncCandidateView,
        installation: &str,
        bindings: &[PeerBinding],
        joining: Option<&JoinCandidate<'_>>,
    ) -> Result<(), String> {
        match self {
            Self::Idp => match joining {
                Some(candidate) => {
                    security_candidates::validate_node_join(view, installation, candidate)
                }
                None => security_candidates::validate(view, installation, bindings),
            },
            Self::IdentityPolicy => security_candidates::validate(view, installation, &[]),
            Self::Devices => security_candidates::validate_device_history(view, bindings, joining),
        }
    }

    pub async fn validate_schema(self, database: &Database) -> io::Result<()> {
        let tables = self
            .tables()
            .into_iter()
            .map(|name| {
                let columns = match self {
                    Self::Idp | Self::IdentityPolicy => public_security_tables()
                        .into_iter()
                        .find(|table| table.name() == name)
                        .expect("public identity table")
                        .columns(),
                    Self::Devices => public_device_columns(name).expect("public Device table"),
                };
                (name, columns)
            })
            .collect::<Vec<_>>();
        let query = tables
            .iter()
            .map(|(table, _)| format!("SELECT * FROM {table} LIMIT 0"))
            .collect::<Vec<_>>()
            .join("; ");
        let results = database
            .client()
            .translate_and_execute(&query, &db::SqlTranslator)
            .await
            .map_err(io::Error::other)?;
        if results.len() != tables.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "missing public schema result",
            ));
        }
        for ((table, expected), result) in tables.into_iter().zip(results) {
            if result.columns.len() != expected.len()
                || result
                    .columns
                    .iter()
                    .any(|column| !expected.contains(&column.name.as_str()))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unreviewed public security schema: {table}"),
                ));
            }
        }
        Ok(())
    }
}

pub struct SecuritySync<'a> {
    idp: &'a Arc<Database>,
    devices: &'a Arc<Database>,
    policy: &'a Arc<Database>,
}

impl<'a> SecuritySync<'a> {
    pub fn new(
        idp: &'a Arc<Database>,
        devices: &'a Arc<Database>,
        policy: &'a Arc<Database>,
    ) -> Self {
        Self {
            idp,
            devices,
            policy,
        }
    }

    pub async fn binding(&self, endpoint: &str, member: Option<Id>) -> io::Result<PeerBinding> {
        let device = DbDeviceRepo::new(self.devices.client())
            .find_approved_by_public_key(endpoint)
            .await
            .map_err(denied)?
            .ok_or_else(|| denied("endpoint has no approved local Device"))?;
        if device.id.is_nil() || member.is_some_and(|member| member.is_nil() || member != device.id)
        {
            return Err(denied("Device and security handshake identity differ"));
        }
        let owner: Id = device.owner_subject.parse().map_err(denied)?;
        if !LocalIdentity::new(self.idp.client())
            .is_active_user(owner)
            .await?
        {
            return Err(denied(
                "Device owner is not a current conflict-clear local User",
            ));
        }
        Ok(PeerBinding {
            member: device.id,
            endpoint: endpoint.to_owned(),
            owner,
        })
    }

    pub async fn authorize(
        &self,
        endpoint: &str,
        member: Id,
        context: SecurityContext,
    ) -> io::Result<()> {
        let binding = self.binding(endpoint, Some(member)).await?;
        self.check_permission(binding.owner, context).await
    }

    pub async fn check_permission(&self, owner: Id, context: SecurityContext) -> io::Result<()> {
        let (action, resource) = context.permission();
        if owner.is_nil()
            || !LocalPermissions::new(self.policy.client())
                .evaluate(&PermissionEvaluationRequest {
                    request_id: Id::now_v7(),
                    subject: PermissionSubject::User { id: owner },
                    action,
                    target: PermissionTarget::Installation { resource },
                })
                .await
                .map_err(denied)?
        {
            return Err(denied(
                "local actor lacks this context's security sync permission",
            ));
        }
        Ok(())
    }
}

fn denied(reason: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, reason.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use db::{Database, Value};
    use idp_model::{contract::IdentityAction, model::Id};
    use management_service::{
        ManagementService,
        replica::{DbPermissionRepo, DbRoleRepo},
    };
    use ofdb_sql::{SyncCandidateView, SyncTableView, SyncValidationDirection};

    use super::{SecurityContext, SecurityHistory, SecuritySync};

    const INSTALLATION: &str = "https://installation.example";

    async fn sql(database: &Database, query: &str) {
        database
            .client()
            .translate_and_execute(query, &db::SqlTranslator)
            .await
            .expect("fixture SQL");
    }

    #[tokio::test]
    async fn histories_keep_schema_selection_with_the_owner() {
        assert_eq!(SecurityContext::Idp.histories(), &[SecurityHistory::Idp]);
        assert_eq!(
            SecurityContext::Device.histories(),
            &[SecurityHistory::IdentityPolicy, SecurityHistory::Devices]
        );
        assert_eq!(SecurityHistory::Idp.name(), "idp");
        assert_eq!(SecurityHistory::IdentityPolicy.name(), "policy-identity");
        assert_eq!(SecurityHistory::Devices.name(), "policy-devices");
        let idp = Database::in_memory();
        idp_model::replica::up(&idp).await.expect("identity schema");
        let devices = Database::in_memory();
        crate::replica::up(&devices).await.expect("Device schema");
        for history in [SecurityHistory::Idp, SecurityHistory::IdentityPolicy] {
            history
                .validate_schema(&idp)
                .await
                .expect("reviewed identity schema");
            assert!(!history.tables().contains(&"devices"));
        }
        SecurityHistory::Devices
            .validate_schema(&devices)
            .await
            .expect("reviewed Device schema");
        assert!(!SecurityHistory::Idp.tables().contains(&"roles"));
        assert!(SecurityHistory::IdentityPolicy.tables().contains(&"roles"));
        sql(&devices, "ALTER TABLE devices ADD COLUMN unreviewed TEXT").await;
        assert!(
            SecurityHistory::Devices
                .validate_schema(&devices)
                .await
                .is_err()
        );
    }

    #[test]
    fn historical_values_reject_secrets_wrong_installation_and_cross_scope_tables() {
        assert!(
            SecurityHistory::Idp
                .validate_value(
                    "idp_replica_members",
                    "issuer",
                    &Value::Text(INSTALLATION.into()),
                    INSTALLATION
                )
                .is_ok()
        );
        assert!(
            SecurityHistory::Idp
                .validate_value(
                    "idp_replica_members",
                    "issuer",
                    &Value::Text("https://other.example".into()),
                    INSTALLATION
                )
                .is_err()
        );
        assert!(
            SecurityHistory::Devices
                .validate_value("devices", "name", &Value::Text("node".into()), INSTALLATION)
                .is_ok()
        );
        assert!(
            SecurityHistory::Devices
                .validate_value(
                    "devices",
                    "enrollment_code_hash",
                    &Value::Null,
                    INSTALLATION
                )
                .is_err()
        );
        assert!(
            SecurityHistory::Devices
                .validate_value(
                    "local_device_enrollments",
                    "device_id",
                    &Value::Null,
                    INSTALLATION
                )
                .is_err()
        );
        assert!(
            SecurityHistory::Idp
                .validate_value("roles", "name", &Value::Text("admin".into()), INSTALLATION)
                .is_err()
        );
        assert!(
            SecurityHistory::IdentityPolicy
                .validate_value("roles", "name", &Value::Text("admin".into()), INSTALLATION)
                .is_ok()
        );
        assert!(
            SecurityHistory::IdentityPolicy
                .validate_value("devices", "name", &Value::Text("node".into()), INSTALLATION)
                .is_err()
        );
    }

    #[test]
    fn candidate_validation_rejects_unreviewed_tables_in_every_history() {
        let view = SyncCandidateView {
            direction: SyncValidationDirection::Incoming,
            candidates: Vec::new(),
            tables: vec![SyncTableView {
                name: "local_device_enrollments".into(),
                columns: Vec::new(),
                rows: Vec::new(),
            }],
        };
        for history in [
            SecurityHistory::Idp,
            SecurityHistory::IdentityPolicy,
            SecurityHistory::Devices,
        ] {
            assert!(
                history
                    .validate_candidate(&view, INSTALLATION, &[], None)
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn current_owner_binding_and_context_permissions_are_rechecked() {
        let idp = Arc::new(Database::in_memory());
        idp_model::replica::up(&idp).await.expect("identity schema");
        let devices = Arc::new(Database::in_memory());
        crate::replica::up(&devices).await.expect("Device schema");
        let actor = Id::now_v7();
        let member = Id::now_v7();
        let endpoint = iroh::SecretKey::generate().public().to_string();
        sql(&idp, &format!("INSERT INTO users (id, handle, name, created_at, updated_at) VALUES (CAST('{actor}' AS UUID), 'owner', 'Owner', 1, 1)")).await;
        sql(&devices, &format!("INSERT INTO devices (id, owner_subject, name, public_key, address, state, approved_at, created_at, updated_at) VALUES (CAST('{member}' AS UUID), '{actor}', 'node', '{endpoint}', 'local', 1, 1, 1, 1)")).await;
        let owner = SecuritySync::new(&idp, &devices, &idp);
        let binding = owner
            .binding(&endpoint, None)
            .await
            .expect("current Device and User");
        assert_eq!(binding.member, member);
        assert_eq!(binding.owner, actor);
        assert!(owner.binding(&endpoint, Some(Id::now_v7())).await.is_err());
        assert!(owner.binding("unknown", None).await.is_err());
        assert!(
            owner
                .authorize(&endpoint, member, SecurityContext::Idp)
                .await
                .is_err()
        );
        ManagementService::new(
            DbPermissionRepo::new(idp.client()),
            DbRoleRepo::new(idp.client()),
        )
        .provision_initial_administrator(
            actor,
            Id::now_v7(),
            &[
                (Id::now_v7(), IdentityAction::DevicePairingRead),
                (Id::now_v7(), IdentityAction::DevicePairingUpdate),
                (Id::now_v7(), IdentityAction::IdpSecuritySync),
            ],
        )
        .await
        .expect("explicit IdP-only grant");
        owner
            .authorize(&endpoint, member, SecurityContext::Idp)
            .await
            .expect("current explicit permission");
        assert!(
            owner
                .authorize(&endpoint, member, SecurityContext::Device)
                .await
                .is_err()
        );
        assert!(
            owner
                .check_permission(Id::nil(), SecurityContext::Idp)
                .await
                .is_err()
        );
        sql(
            &devices,
            &format!(
                "UPDATE devices SET state = 2, revoked_at = 2 WHERE id = CAST('{member}' AS UUID)"
            ),
        )
        .await;
        assert!(
            owner
                .authorize(&endpoint, member, SecurityContext::Idp)
                .await
                .is_err()
        );
        sql(&devices, &format!("UPDATE devices SET state = 1, revoked_at = NULL WHERE id = CAST('{member}' AS UUID)")).await;
        sql(
            &idp,
            &format!("DELETE FROM users WHERE id = CAST('{actor}' AS UUID)"),
        )
        .await;
        assert!(owner.binding(&endpoint, Some(member)).await.is_err());
    }
}
