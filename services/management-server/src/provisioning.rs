use std::{io, sync::Arc};

use of_model::id::Id;
use of_model::idp::contract::permission_evaluation::IdentityAction;
use ofdb_sql::Database;

pub async fn identity_client(
    config: &crate::config::app_config::AppConfig,
) -> io::Result<ofdb_sql::Client> {
    config.validate_identity_connection()?;
    if let Some(remote) = &config.identity_database {
        let client = remote.connect().await?;
        db::ofdb::validators::validate_identity(&client)
            .await
            .map_err(io::Error::other)?;
        return Ok(client);
    }
    #[cfg(unix)]
    {
        let deadline = std::time::Duration::from_millis(config.identity_socket_deadline_ms);
        tokio::time::timeout(deadline, async {
            let client = ofdb_sql::Client::connect_unix(
                config
                    .identity_socket
                    .as_ref()
                    .expect("validated socket selector"),
            )
            .await
            .map_err(io::Error::other)?
            .with_deadline(deadline);
            db::ofdb::validators::validate_identity(&client)
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
    actions: &[IdentityAction],
) -> io::Result<()> {
    management_service::provision_initial_administrator(&engine.client(), user_id, actions)
        .await
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_identity_uses_explicit_socket_without_opening_files() {
        use std::{sync::Arc, time::Duration};
        let root =
            std::env::temp_dir().join(format!("management-socket-{}", of_model::id::Id::now_v7()));
        std::fs::create_dir(&root).expect("private test root");
        let socket = root.join("identity.sock");
        let engine =
            Arc::new(ofdb_sql::Database::open(root.join("owner.redb")).expect("owner database"));
        db::ofdb::up(&engine.client())
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
        let config = crate::config::app_config::AppConfig {
            data_dir: root.join("consumer").to_string_lossy().into_owned(),
            identity_socket: Some(socket),
            identity_socket_deadline_ms: 1000,
            ..Default::default()
        };
        let client = crate::provisioning::identity_client(&config)
            .await
            .expect("explicit socket identity connection");
        db::ofdb::validators::validate_identity(&client)
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
            of_model::id::Id::now_v7()
        ));
        std::fs::create_dir(&root).expect("test root");
        let mut config = crate::config::app_config::AppConfig {
            data_dir: root.join("consumer").to_string_lossy().into_owned(),
            identity_socket: Some(root.join("missing.sock")),
            identity_socket_deadline_ms: 50,
            ..Default::default()
        };
        assert!(crate::provisioning::identity_client(&config).await.is_err());
        std::fs::write(root.join("not-a-socket"), b"retained evidence").expect("ordinary file");
        config.identity_socket = Some(root.join("not-a-socket"));
        assert!(crate::provisioning::identity_client(&config).await.is_err());
        assert_eq!(
            std::fs::read(root.join("not-a-socket")).expect("file untouched"),
            b"retained evidence"
        );
        config.identity_socket = Some(root.join("stall.sock"));
        let listener =
            tokio::net::UnixListener::bind(config.identity_socket.as_ref().expect("socket"))
                .expect("stalled listener");
        let started = Instant::now();
        assert!(crate::provisioning::identity_client(&config).await.is_err());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "stalled socket must be bounded"
        );
        drop(listener);
        config.identity_database = Some(db::remote::RemoteDatabaseConfig {
            endpoint: "http://127.0.0.1:50051".into(),
            ca_certificate: None,
            client_certificate: None,
            client_key: None,
            deadline_ms: 100,
        });
        assert_eq!(
            crate::provisioning::identity_client(&config)
                .await
                .expect_err("ambiguous selectors")
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        config.identity_database = None;
        for deadline in [0, 300_001] {
            config.identity_socket_deadline_ms = deadline;
            assert_eq!(
                crate::provisioning::identity_client(&config)
                    .await
                    .expect_err("invalid deadline")
                    .kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
        config.identity_socket_deadline_ms = 1000;
        config.identity_socket = Some("relative.sock".into());
        assert_eq!(
            crate::provisioning::identity_client(&config)
                .await
                .expect_err("relative socket")
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        let engine = Arc::new(
            ofdb_sql::Database::open(root.join("wrong-schema.redb")).expect("uninitialized owner"),
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
            crate::provisioning::identity_client(&config).await.is_err(),
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
        let root =
            std::env::temp_dir().join(format!("management-client-{}", of_model::id::Id::now_v7()));
        let mut config = crate::config::app_config::AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert!(crate::provisioning::identity_client(&config).await.is_err());
        config.identity_database = Some(db::remote::RemoteDatabaseConfig {
            endpoint: "http://sql.example:50051".into(),
            ca_certificate: None,
            client_certificate: None,
            client_key: None,
            deadline_ms: 100,
        });
        assert!(crate::provisioning::identity_client(&config).await.is_err());
        assert!(!root.exists());
    }
}

#[cfg(test)]
mod initial_administrator_tests {
    use std::sync::Arc;

    use management_service::LocalPermissions;
    use of_model::id::Id;
    use of_model::idp::contract::permission_evaluation::IdentityAction;
    use of_model::idp::contract::permission_evaluation::IdentityResource;
    use of_model::idp::contract::permission_evaluation::PermissionEvaluationRequest;
    use of_model::idp::contract::permission_evaluation::PermissionSubject;
    use of_model::idp::contract::permission_evaluation::PermissionTarget;
    use ofdb_sql::Database;

    use crate::provisioning::provision_initial_administrator;

    #[tokio::test]
    async fn initial_administrator_setup_retries_after_reopen_without_changing_grants() {
        let path = std::env::temp_dir().join(format!("administrator-grant-{}.redb", Id::now_v7()));
        let user = Id::now_v7();
        let actions = management_service::INITIAL_ADMINISTRATOR_ACTIONS;
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
            db::ofdb::up(&engine.client())
                .await
                .expect("identity schema");
            provision_initial_administrator(engine, user, &actions)
                .await
                .expect("setup grants");
        }
        {
            let engine = Arc::new(Database::open(&path).expect("offline reopen"));
            provision_initial_administrator(Arc::clone(&engine), user, &actions)
                .await
                .expect("exact retry");
            assert!(
                provision_initial_administrator(Arc::clone(&engine), Id::now_v7(), &actions)
                    .await
                    .is_err()
            );
            assert!(
                provision_initial_administrator(
                    Arc::clone(&engine),
                    user,
                    &[
                        IdentityAction::DevicePairingRead,
                        IdentityAction::DevicePairingUpdate
                    ]
                )
                .await
                .is_err()
            );
            assert!(
                LocalPermissions::new(engine.client())
                    .evaluate(&request)
                    .await
                    .expect("offline permission")
            );
        }
        std::fs::remove_file(path).expect("remove database");
    }
}
