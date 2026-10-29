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
    config.validate_identity_connection()?;
    if let Some(remote) = &config.identity_database {
        let client = remote.connect().await?;
        idp_model::replica::validate_identity(&client)
            .await
            .map_err(io::Error::other)?;
        return Ok(client);
    }
    #[cfg(unix)]
    {
        let deadline = std::time::Duration::from_millis(config.identity_socket_deadline_ms);
        tokio::time::timeout(deadline, async {
            let client = db::Client::connect_unix(
                config
                    .identity_socket
                    .as_ref()
                    .expect("validated socket selector"),
            )
            .await
            .map_err(io::Error::other)?
            .with_deadline(deadline);
            idp_model::replica::validate_identity(&client)
                .await
                .map_err(io::Error::other)?;
            Ok(client)
        })
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "identity socket connection/schema validation deadline exceeded",
            )
        })?
    }
    #[cfg(not(unix))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "identity_socket requires Unix",
        ))
    }
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
    #[test]
    fn administrator_inventory_reopens_current_seeds_and_rejects_old_seeds_without_mutation() {
        let fresh = super::InitialAdministratorGrant::fresh();
        let retained = serde_json::to_value(&fresh).expect("retain current inventory");
        assert_eq!(
            retained["permissions"]
                .as_array()
                .expect("permission IDs")
                .len(),
            management_service::INITIAL_ADMINISTRATOR_ACTIONS.len()
        );
        let reopened: super::InitialAdministratorGrant =
            serde_json::from_value(retained.clone()).expect("reopen current inventory");
        reopened
            .validate()
            .expect("fresh reset/Create inventory survives restart");
        assert_eq!(
            serde_json::to_value(&reopened).expect("unchanged inventory"),
            retained
        );
        let mut old = retained;
        old["permissions"]
            .as_array_mut()
            .expect("old permission IDs")
            .pop()
            .expect("remove newly added permission");
        let rejected: super::InitialAdministratorGrant =
            serde_json::from_value(old.clone()).expect("decode old inventory");
        assert_eq!(
            rejected
                .validate()
                .expect_err("old inventory is not migrated")
                .kind(),
            std::io::ErrorKind::InvalidData
        );
        assert_eq!(
            serde_json::to_value(&rejected).expect("preserve old evidence"),
            old
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_identity_uses_explicit_socket_without_opening_files() {
        use std::{sync::Arc, time::Duration};
        let root = std::env::temp_dir().join(format!(
            "management-socket-{}",
            idp_model::model::Id::now_v7()
        ));
        std::fs::create_dir(&root).expect("private test root");
        let socket = root.join("identity.sock");
        let engine = Arc::new(db::Database::open(root.join("owner.redb")).expect("owner database"));
        idp_model::replica::up(&engine)
            .await
            .expect("identity schema");
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let serving = engine.clone();
        let path = socket.clone();
        let task = tokio::spawn(async move {
            serving
                .serve_unix(path, async {
                    let _ = stopped.await;
                })
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while !socket.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("SQL listener startup");
        let config = crate::AppConfig {
            data_dir: root.join("consumer").to_string_lossy().into_owned(),
            identity_socket: Some(socket),
            identity_socket_deadline_ms: 1000,
            ..Default::default()
        };
        let client = super::identity_client(&config)
            .await
            .expect("explicit socket identity connection");
        idp_model::replica::validate_identity(&client)
            .await
            .expect("same identity schema");
        assert!(
            !root.join("consumer").exists(),
            "consumer must not open a local owner"
        );
        drop(client);
        stop.send(()).expect("stop SQL listener");
        task.await.expect("SQL task").expect("SQL listener");
        drop(engine);
        std::fs::remove_dir_all(root).expect("test cleanup");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_socket_rejects_invalid_selectors_missing_files_stalls_and_wrong_schema() {
        use std::{
            sync::Arc,
            time::{Duration, Instant},
        };
        let root = std::env::temp_dir().join(format!(
            "management-socket-denial-{}",
            idp_model::model::Id::now_v7()
        ));
        std::fs::create_dir(&root).expect("test root");
        let mut config = crate::AppConfig {
            data_dir: root.join("consumer").to_string_lossy().into_owned(),
            identity_socket: Some(root.join("missing.sock")),
            identity_socket_deadline_ms: 50,
            ..Default::default()
        };
        assert!(super::identity_client(&config).await.is_err());
        std::fs::write(root.join("not-a-socket"), b"retained evidence").expect("ordinary file");
        config.identity_socket = Some(root.join("not-a-socket"));
        assert!(super::identity_client(&config).await.is_err());
        assert_eq!(
            std::fs::read(root.join("not-a-socket")).expect("file untouched"),
            b"retained evidence"
        );
        config.identity_socket = Some(root.join("stall.sock"));
        let listener =
            tokio::net::UnixListener::bind(config.identity_socket.as_ref().expect("socket"))
                .expect("stalled listener");
        let started = Instant::now();
        assert!(super::identity_client(&config).await.is_err());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "stalled socket must be bounded"
        );
        drop(listener);
        config.identity_database = Some(db::RemoteDatabaseConfig {
            endpoint: "http://127.0.0.1:50051".into(),
            ca_certificate: None,
            client_certificate: None,
            client_key: None,
            deadline_ms: 100,
        });
        assert_eq!(
            super::identity_client(&config)
                .await
                .expect_err("ambiguous selectors")
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        config.identity_database = None;
        for deadline in [0, 300_001] {
            config.identity_socket_deadline_ms = deadline;
            assert_eq!(
                super::identity_client(&config)
                    .await
                    .expect_err("invalid deadline")
                    .kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
        config.identity_socket_deadline_ms = 1000;
        config.identity_socket = Some("relative.sock".into());
        assert_eq!(
            super::identity_client(&config)
                .await
                .expect_err("relative socket")
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        let engine = Arc::new(
            db::Database::open(root.join("wrong-schema.redb")).expect("uninitialized owner"),
        );
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let serving = engine.clone();
        let socket = root.join("wrong-schema.sock");
        let path = socket.clone();
        let task = tokio::spawn(async move {
            serving
                .serve_unix(path, async {
                    let _ = stopped.await;
                })
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while !socket.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("SQL listener");
        config.identity_socket = Some(socket);
        assert!(
            super::identity_client(&config).await.is_err(),
            "socket transport must not bypass identity schema validation"
        );
        assert!(!root.join("consumer").exists());
        stop.send(()).expect("stop SQL");
        task.await.expect("SQL task").expect("SQL shutdown");
        drop(engine);
        std::fs::remove_dir_all(root).expect("test cleanup");
    }

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
