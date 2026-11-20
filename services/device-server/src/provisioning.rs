use std::{
    fs, io,
    path::{Component, Path},
    sync::Arc,
};

use ofdb_sql::Database;

use ofdb_sql::SqlTranslator;

use device_service::ofdb::DbSelectionPolicyRepo;
use of_model::device::selection_policy::SelectionPolicy;
use of_model::id::Id;
use ofdb_sql::Value;

use crate::AppConfig;

impl AppConfig {
    pub fn require_provisioned_device(&self) -> io::Result<()> {
        let path = Path::new(&self.data_dir).join("devices.redb");
        if path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "empty paths and parent components are not allowed",
            ));
        }
        let absolute = if path.is_absolute() {
            path.clone()
        } else {
            std::env::current_dir()?.join(&path)
        };
        let absolute = absolute
            .components()
            .filter(|part| !matches!(part, Component::CurDir))
            .collect::<std::path::PathBuf>();
        for ancestor in absolute.ancestors() {
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "symlink ancestry is not allowed",
                    ));
                }
                Ok(_) => (),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
        }
        if !fs::symlink_metadata(&path)?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "input or retained state must be a regular file",
            ));
        }
        if !fs::File::open(path)?.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "input or retained state must be a regular file",
            ));
        }
        Ok(())
    }
}

pub async fn open_device_database(config: &AppConfig) -> io::Result<Arc<Database>> {
    if config.device_database.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local Device owner and remote SQL are mutually exclusive",
        ));
    }
    open_local_device_database(Path::new(&config.data_dir)).await
}

/// Opens and migrates local state; retained-state callers must preflight before this creating open.
pub async fn open_local_device_database(directory: &Path) -> io::Result<Arc<Database>> {
    fs::create_dir_all(directory)?;
    let engine =
        Arc::new(Database::open(directory.join("devices.redb")).map_err(io::Error::other)?);
    device_service::ofdb::up(&engine)
        .await
        .map_err(io::Error::other)?;
    Ok(engine)
}

pub async fn provision_initial_device(
    engine: ofdb_sql::Client,
    retained_id: Option<Id>,
    owner: Id,
    endpoint_id: &str,
    name: &str,
) -> io::Result<Id> {
    let canonical_endpoint = endpoint_id
        .parse::<iroh::EndpointId>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid initial endpoint ID"))?;
    if owner.is_nil()
        || canonical_endpoint.to_string() != endpoint_id
        || name.trim().is_empty()
        || name.len() > 512
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "initial Device requires stable owner, canonical endpoint and name",
        ));
    }
    let mut transaction = engine.transaction().await.map_err(io::Error::other)?;
    let devices = transaction
        .translate_and_execute(
            "SELECT id, owner_subject, name, public_key, state, revoked_at FROM devices",
            &SqlTranslator,
        )
        .await
        .map_err(io::Error::other)?;
    let expected = [
        Value::Text(owner.to_string()),
        Value::Text(name.into()),
        Value::Text(endpoint_id.into()),
        Value::Integer(1),
        Value::Null,
    ];
    let device_id = match devices[0].rows.as_slice() {
        [] if retained_id.is_none() => {
            let result = transaction.translate_and_execute_with_params(
                "INSERT INTO devices (owner_subject, name, public_key, address, state, approved_at, revoked_at, created_at, updated_at) VALUES (?, ?, ?, '', 1, 1, NULL, 1, 1) RETURNING id",
                Some(&ofdb_sql::QueryParams::Positional(expected[..3].to_vec())),
                &SqlTranslator,
            ).await.map_err(io::Error::other)?;
            let Some(Value::Uuid(id)) = result
                .first()
                .and_then(|result| result.rows.first())
                .and_then(|row| row.values.first())
            else {
                return Err(io::Error::other("missing generated Device ID"));
            };
            *id
        }
        [row] if matches!(row.values.as_slice(), [Value::Uuid(_), fields @ ..] if fields == expected) =>
        {
            let Value::Uuid(id) = row.values[0] else {
                return Err(io::Error::other("invalid retained Device ID"));
            };
            if retained_id.is_some_and(|retained| retained != id) {
                return Err(io::Error::other("initial Device ID changed"));
            }
            id
        }
        _ => {
            return Err(io::Error::other(
                "initial Node Device conflicts with persisted setup",
            ));
        }
    };
    for table in ["devices", "local_device_enrollments"] {
        if !transaction
            .row_conflicts(
                table,
                &device_service::ofdb::device_repo::device_key(table, device_id),
            )
            .await
            .map_err(io::Error::other)?
            .is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "initial Device has unresolved owner conflicts",
            ));
        }
    }
    transaction.commit().await.map_err(io::Error::other)?;

    let policies = DbSelectionPolicyRepo::new(engine.clone());
    let expected = SelectionPolicy {
        device_id,
        owner_subject: owner.to_string(),
        application_id: None,
        selected_kind: None,
        selected_id: None,
        admin_allowed: true,
    };
    if let Some(existing) = policies
        .get(device_id, &expected.owner_subject, Id::nil())
        .await
        .map_err(io::Error::other)?
    {
        if existing != expected
            || !policies
                .selected_resources_for_device(device_id)
                .await
                .map_err(io::Error::other)?
                .is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "initial Device policy conflicts with persisted setup",
            ));
        }
        return Ok(device_id);
    }
    policies
        .set_prevalidated(expected)
        .await
        .map_err(io::Error::other)?;
    Ok(device_id)
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use of_model::id::Id;

    use crate::AppConfig;

    #[tokio::test]
    async fn initial_provisioning_returns_stable_generated_device_id() {
        let database = ofdb_sql::Database::in_memory();
        device_service::ofdb::up(&database)
            .await
            .expect("Device schema");
        let owner = Id::now_v7();
        let endpoint = iroh::SecretKey::generate().public().to_string();
        let client = database.client();
        let provision = |retained| {
            super::provision_initial_device(client.clone(), retained, owner, &endpoint, "Node")
        };
        let id = provision(None).await.expect("first provision");
        assert_eq!(id.get_version_num(), 7);
        assert_eq!(provision(None).await.expect("retry before retention"), id);
        assert_eq!(
            provision(Some(id)).await.expect("retry after retention"),
            id
        );
        assert!(provision(Some(Id::now_v7())).await.is_err());
    }

    #[test]
    fn retained_device_preflight_refuses_missing_roots_without_replacement() {
        let root = std::env::temp_dir().join(format!("device-retained-{}", Id::now_v7()));
        let config = AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert_eq!(
            config
                .require_provisioned_device()
                .expect_err("missing root")
                .kind(),
            io::ErrorKind::NotFound
        );
        assert!(!root.exists());
        fs::create_dir(&root).expect("retained root");
        let path = root.join("devices.redb");
        assert_eq!(
            config
                .require_provisioned_device()
                .expect_err("missing database")
                .kind(),
            io::ErrorKind::NotFound
        );
        assert!(!path.exists());
        fs::create_dir(&path).expect("non-file retained database");
        let error = config
            .require_provisioned_device()
            .expect_err("not a regular file");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "input or retained state must be a regular file"
        );
        fs::remove_dir(&path).expect("remove malformed fixture");
        fs::write(&path, b"retained bytes").expect("retained file");
        config
            .require_provisioned_device()
            .expect("existing-only preflight does not migrate");
        assert_eq!(
            fs::read(&path).expect("unchanged retained file"),
            b"retained bytes"
        );
        #[cfg(unix)]
        {
            let linked = root.with_extension("link");
            std::os::unix::fs::symlink(&root, &linked).expect("symlink root");
            let linked_config = AppConfig {
                data_dir: linked.to_string_lossy().into_owned(),
                ..Default::default()
            };
            let error = linked_config
                .require_provisioned_device()
                .expect_err("symlink ancestry");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(error.to_string(), "symlink ancestry is not allowed");
            fs::remove_file(linked).expect("remove symlink fixture");
        }
        let parent_config = AppConfig {
            data_dir: root
                .join("..")
                .join("other-device")
                .to_string_lossy()
                .into_owned(),
            ..Default::default()
        };
        let error = parent_config
            .require_provisioned_device()
            .expect_err("parent components refused before file access");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "empty paths and parent components are not allowed"
        );
        fs::remove_dir_all(root).expect("remove retained fixture");
    }

    #[tokio::test]
    async fn local_device_open_initializes_schema_and_reopens_through_owner() {
        use device_service::{device_repo::DeviceRepo, replica::device_repo::DbDeviceRepo};

        let root = std::env::temp_dir().join(format!("device-open-{}", Id::now_v7()));
        let config = AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            ..Default::default()
        };
        {
            let database = crate::provisioning::open_local_device_database(&root)
                .await
                .expect("initialize local Device owner");
            assert!(
                !DbDeviceRepo::new(database.client())
                    .has_any()
                    .await
                    .expect("initialized Device schema")
            );
        }
        config
            .require_provisioned_device()
            .expect("retained Device store");
        {
            let database = crate::provisioning::open_device_database(&config)
                .await
                .expect("reopen configured Device owner");
            assert!(
                !DbDeviceRepo::new(database.client())
                    .has_any()
                    .await
                    .expect("existing Device schema")
            );
        }
        fs::remove_file(root.join("devices.redb")).expect("lose retained database");
        assert_eq!(
            config
                .require_provisioned_device()
                .expect_err("no replacement")
                .kind(),
            io::ErrorKind::NotFound
        );
        assert!(!root.join("devices.redb").exists());
        fs::remove_dir_all(root).expect("remove Device fixture");
    }

    #[tokio::test]
    async fn remote_device_startup_never_falls_back_to_a_private_database() {
        let root = std::env::temp_dir().join(format!(
            "remote-device-startup-{}",
            of_model::id::Id::now_v7()
        ));
        let config: crate::AppConfig = serde_json::from_value(serde_json::json!({
            "data_dir": root,
            "device_database": {"endpoint": "http://127.0.0.1:1", "deadline_ms": 100}
        }))
        .expect("remote startup configuration");
        assert!(
            crate::provisioning::open_device_database(&config)
                .await
                .is_err(),
            "remote configuration cannot open a local owner"
        );
        assert!(
            crate::database_client::device_client(&config)
                .await
                .is_err(),
            "unavailable remote owner must fail startup"
        );
        assert!(
            !root.exists(),
            "failed remote startup must leave no private database"
        );
    }
}

pub(crate) fn open_identity_database(config: &AppConfig) -> io::Result<Arc<Database>> {
    let path = config.identity_database_path.as_deref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "identity_database_path is required for local Device startup",
        )
    })?;
    if !Path::new(path).is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "identity database must already exist",
        ));
    }
    Database::open(path).map(Arc::new).map_err(io::Error::other)
}
