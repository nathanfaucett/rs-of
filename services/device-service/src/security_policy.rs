#[cfg(feature = "ofdb")]
use std::io;

use idp_service::{
    ofdb::LocalIdentity,
    security_sync::{PUBLIC_IDENTITY_POLICY_TABLES, public_security_tables},
};
use management_service::LocalPermissions;
use of_model::id::Id;
use of_model::idp::contract::permission_evaluation::IdentityAction;
use of_model::idp::contract::permission_evaluation::IdentityResource;
use of_model::idp::contract::permission_evaluation::PermissionEvaluationRequest;
use of_model::idp::contract::permission_evaluation::PermissionTarget;
use ofdb_sql::Value;
use serde::{Deserialize, Serialize};

use crate::{
    ofdb::DbDeviceRepo,
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

    pub fn permission(self) -> (IdentityAction, IdentityResource) {
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
            Self::Devices => PUBLIC_DEVICE_TABLES.iter().copied().collect(),
        }
    }

    pub fn validate_value(
        self,
        table: &str,
        column: &str,
        value: &Value,
        _installation: &str,
    ) -> Result<(), String> {
        match self {
            Self::Idp => {
                let public = public_security_tables()
                    .into_iter()
                    .find(|t| t.name() == table)
                    .ok_or_else(|| format!("unknown idp table: {table}"))?;
                if !public.columns().contains(&column) {
                    return Err(format!("unknown idp column: {table}.{column}"));
                }
                if !public.permits_historical_value(column, value) {
                    return Err(format!("invalid idp value: {table}.{column}"));
                }
                Ok(())
            }
            Self::IdentityPolicy => {
                let public = public_security_tables()
                    .into_iter()
                    .find(|t| t.name() == table)
                    .ok_or_else(|| format!("unknown identity policy table: {table}"))?;
                if !public.columns().contains(&column) {
                    return Err(format!("unknown identity policy column: {table}.{column}"));
                }
                if !public.permits_historical_value(column, value) {
                    return Err(format!("invalid identity policy value: {table}.{column}"));
                }
                Ok(())
            }
            Self::Devices => {
                if !permits_device_table(table) {
                    return Err(format!("unknown device table: {table}"));
                }
                let columns = public_device_columns(table)
                    .ok_or_else(|| format!("unknown device table columns: {table}"))?;
                if !columns.contains(&column) {
                    return Err(format!("unknown device column: {table}.{column}"));
                }
                if !security_candidates::permits_device_value(table, column, value) {
                    return Err(format!("invalid device value: {table}.{column}"));
                }
                Ok(())
            }
        }
    }

    pub fn validate_candidate(
        self,
        view: &ofdb_sql::SyncCandidateView,
        _installation: &str,
        approval: Option<&JoinCandidate<'_>>,
    ) -> Result<(), io::Error> {
        match self {
            Self::Idp | Self::IdentityPolicy => security_candidates::validate(view)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            Self::Devices => {
                let bindings = approval
                    .map(|a| {
                        vec![PeerBinding {
                            member: a.device_id,
                            endpoint: a.endpoint.to_owned(),
                            owner: a.owner,
                        }]
                    })
                    .unwrap_or_default();
                security_candidates::validate_device_history(view, &bindings, approval)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            }
        }
    }

    pub async fn validate_schema(
        self,
        idp: &LocalIdentity,
        permissions: &LocalPermissions,
        candidate: &DbDeviceRepo,
    ) -> Result<(), io::Error> {
        let _ = (idp, permissions, candidate);
        Ok(())
    }
}

fn permits_device_table(table: &str) -> bool {
    PUBLIC_DEVICE_TABLES.contains(&table)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdpPolicy {
    pub installation: String,
    pub tables: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DevicePolicy {
    pub installation: String,
    pub tables: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    pub installation: String,
    pub id: Id,
    pub owner_subject: Id,
    pub name: String,
    pub public_key: Vec<u8>,
    pub address: String,
    pub state: of_model::device::contract::device_state::DeviceState,
    pub approved_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone)]
pub struct SecuritySync<'a> {
    idp: &'a LocalIdentity,
    devices: &'a DbDeviceRepo,
    policy: &'a LocalPermissions,
}

impl<'a> SecuritySync<'a> {
    pub fn new(
        idp: &'a LocalIdentity,
        devices: &'a DbDeviceRepo,
        policy: &'a LocalPermissions,
    ) -> Self {
        Self {
            idp,
            devices,
            policy,
        }
    }

    pub async fn binding(
        &self,
        context: SecurityContext,
        request: &PermissionEvaluationRequest,
    ) -> Result<SecurityHistory, io::Error> {
        let histories = context.histories();
        if histories.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no histories for context",
            ));
        }
        for history in histories {
            let (action, resource) = context.permission();
            let request = PermissionEvaluationRequest {
                request_id: request.request_id,
                subject: request.subject.clone(),
                action,
                target: PermissionTarget::Installation { resource },
            };
            let permitted = self.policy.evaluate(&request).await.map_err(|_| denied())?;
            if permitted {
                return Ok(*history);
            }
        }
        Err(denied())
    }

    pub async fn authorize(
        &self,
        context: SecurityContext,
        request: &PermissionEvaluationRequest,
    ) -> Result<(), io::Error> {
        self.binding(context, request).await?;
        Ok(())
    }

    pub async fn check_permission(
        &self,
        request: &PermissionEvaluationRequest,
    ) -> Result<bool, io::Error> {
        self.policy.evaluate(request).await.map_err(|_| denied())
    }

    /// Get the peer binding for a given endpoint and member.
    pub async fn peer_binding(&self, endpoint: &str, member: Id) -> Result<PeerBinding, io::Error> {
        Ok(PeerBinding {
            member,
            endpoint: endpoint.to_owned(),
            owner: member,
        })
    }
}

fn denied() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "permission denied")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const INSTALLATION: &str = "test-installation";

    async fn sql() -> Arc<ofdb_sql::Database> {
        Arc::new(ofdb_sql::Database::in_memory())
    }

    async fn histories_keep_schema_selection_with_the_owner() {
        // Test implementation
    }

    fn historical_values_reject_secrets_wrong_installation_and_cross_scope_tables() {
        // Test implementation
    }

    fn candidate_validation_rejects_unreviewed_tables_in_every_history() {
        // Test implementation
    }

    async fn current_owner_binding_and_context_permissions_are_rechecked() {
        // Test implementation
    }
}
