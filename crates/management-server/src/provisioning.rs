use std::{io, sync::Arc};

use db::Database;
use idp_model::{
    contract::IdentityAction,
    model::{Id, Role},
};
use management_service::{
    INITIAL_ADMINISTRATOR_ACTIONS, ManagementService,
    replica::{DbPermissionRepo, DbRoleRepo},
};

#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
pub struct InitialAdministratorGrant {
    role: Id,
    permissions: Vec<Id>,
}

impl InitialAdministratorGrant {
    pub fn fresh() -> Self {
        Self {
            role: Id::now_v7(),
            permissions: INITIAL_ADMINISTRATOR_ACTIONS
                .iter()
                .map(|_| Id::now_v7())
                .collect(),
        }
    }

    pub fn identities(&self) -> impl Iterator<Item = Id> + '_ {
        core::iter::once(self.role).chain(self.permissions.iter().copied())
    }

    pub fn validate(&self) -> io::Result<()> {
        let mut identities = self.identities().collect::<Vec<_>>();
        identities.sort();
        identities.dedup();
        if self.permissions.len() != INITIAL_ADMINISTRATOR_ACTIONS.len()
            || identities.len() != self.permissions.len() + 1
            || identities.iter().any(Id::is_nil)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid retained administrator permission inventory",
            ));
        }
        Ok(())
    }

    pub async fn provision(&self, engine: Arc<Database>, user: Id) -> io::Result<Role> {
        self.validate()?;
        let permissions = self
            .permissions
            .iter()
            .copied()
            .zip(INITIAL_ADMINISTRATOR_ACTIONS)
            .collect::<Vec<_>>();
        provision_initial_administrator(engine, user, self.role, &permissions).await
    }
}

pub async fn identity_client(config: &crate::AppConfig) -> io::Result<db::Client> {
    let remote = config.identity_database.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "standalone Management requires identity_database; local composition injects the identity client",
        )
    })?;
    let client = remote.connect().await?;
    idp_model::replica::validate_identity(&client)
        .await
        .map_err(io::Error::other)?;
    Ok(client)
}

pub async fn provision_initial_administrator(
    engine: Arc<Database>,
    user_id: Id,
    role_id: Id,
    permissions: &[(Id, IdentityAction)],
) -> io::Result<Role> {
    let service = ManagementService::new(
        DbPermissionRepo::new(engine.client()),
        DbRoleRepo::new(engine.client()),
    );
    service
        .provision_initial_administrator(user_id, role_id, permissions)
        .await
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn standalone_identity_requires_remote_without_local_fallback() {
        let root = std::env::temp_dir().join(format!(
            "management-client-{}",
            idp_model::model::Id::now_v7()
        ));
        let mut config = crate::AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert!(super::identity_client(&config).await.is_err());
        config.identity_database = Some(db::RemoteDatabaseConfig {
            endpoint: "http://sql.example:50051".into(),
            ca_certificate: None,
            client_certificate: None,
            client_key: None,
            deadline_ms: 100,
        });
        assert!(super::identity_client(&config).await.is_err());
        assert!(!root.exists());
    }
}

#[cfg(test)]
mod initial_administrator_tests {
    use std::sync::Arc;

    use db::Database;
    use idp_model::{
        contract::{
            IdentityAction, IdentityResource, PermissionEvaluationRequest, PermissionSubject,
            PermissionTarget,
        },
        model::Id,
    };
    use management_service::LocalPermissions;

    use super::InitialAdministratorGrant;

    #[tokio::test]
    async fn retained_grant_retries_exactly_and_reopens_offline() {
        let path = std::env::temp_dir().join(format!("administrator-grant-{}.redb", Id::now_v7()));
        let user = Id::now_v7();
        let grant = InitialAdministratorGrant::fresh();
        let retained = serde_json::to_value(&grant).expect("retained grant");
        let request = PermissionEvaluationRequest {
            request_id: Id::now_v7(),
            subject: PermissionSubject::User { id: user },
            action: IdentityAction::DevicePairingUpdate,
            target: PermissionTarget::Installation {
                resource: IdentityResource::DevicePairing,
            },
        };
        {
            let engine = Arc::new(Database::open(&path).expect("owner database"));
            idp_model::replica::up(&engine)
                .await
                .expect("identity schema");
            grant
                .provision(Arc::clone(&engine), user)
                .await
                .expect("grant administrator");
            grant
                .provision(Arc::clone(&engine), user)
                .await
                .expect("exact retry");
            assert!(
                LocalPermissions::new(engine.client())
                    .evaluate(&request)
                    .await
                    .expect("explicit pairing grant")
            );
            let mut changed = retained.clone();
            changed["permissions"][0] = serde_json::json!(Id::now_v7());
            let changed: InitialAdministratorGrant =
                serde_json::from_value(changed).expect("conflicting grant");
            assert!(changed.provision(Arc::clone(&engine), user).await.is_err());
        }
        {
            let engine = Arc::new(Database::open(&path).expect("offline reopen"));
            let grant: InitialAdministratorGrant =
                serde_json::from_value(retained.clone()).expect("retained grant");
            grant.validate().expect("retained inventory");
            assert_eq!(
                serde_json::to_value(&grant).expect("unchanged grant"),
                retained
            );
            assert!(
                LocalPermissions::new(engine.client())
                    .evaluate(&request)
                    .await
                    .expect("offline permission")
            );
        }
        for permissions in [serde_json::json!([]), serde_json::json!([Id::nil()])] {
            let mut malformed = retained.clone();
            malformed["permissions"] = permissions;
            let malformed: InitialAdministratorGrant =
                serde_json::from_value(malformed).expect("decode malformed inventory");
            assert!(malformed.validate().is_err());
        }
        std::fs::remove_file(path).expect("remove database");
    }
}
