use std::{
    fs, io,
    path::{Component, Path},
    sync::Arc,
};

use db::{Database, Row, SqlTranslator, Value};
use device_service::replica::{DbSelectionPolicyRepo, SelectionPolicy};
use idp_model::model::Id;

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
    device_service::replica::up(&engine)
        .await
        .map_err(io::Error::other)?;
    Ok(engine)
}

pub async fn provision_initial_device(
    engine: db::Client,
    device_id: Id,
    owner: Id,
    endpoint_id: &str,
    name: &str,
) -> io::Result<()> {
    let canonical_endpoint = endpoint_id
        .parse::<iroh::EndpointId>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid initial endpoint ID"))?;
    if device_id.is_nil()
        || owner.is_nil()
        || canonical_endpoint.to_string() != endpoint_id
        || name.trim().is_empty()
        || name.len() > 512
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "initial Device requires stable owner, Device ID, canonical endpoint and name",
        ));
    }
    let mut transaction = engine.transaction().await.map_err(io::Error::other)?;
    for table in ["devices", "local_device_enrollments"] {
        if !transaction
            .row_conflicts(table, &Row::new(vec![Value::Uuid(device_id)]))
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
    let devices = transaction
        .translate_and_execute(
            "SELECT id, owner_subject, name, public_key, state, revoked_at FROM devices",
            &SqlTranslator,
        )
        .await
        .map_err(io::Error::other)?;
    if devices[0].rows.is_empty() {
        let name_esc = name.replace("'", "''");
        let endpoint_esc = endpoint_id.replace("'", "''");
        let query = format!(
            "INSERT INTO devices (id, owner_subject, name, public_key, address, state, approved_at, revoked_at, created_at, updated_at) VALUES (CAST('{device_id}' AS UUID), '{owner}', '{name_esc}', '{endpoint_esc}', '', 1, 1, NULL, 1, 1)",
        );
        transaction
            .translate_and_execute(&query, &SqlTranslator)
            .await
            .map_err(io::Error::other)?;
    } else if devices[0].rows.len() != 1
        || devices[0].rows[0].values
            != [
                Value::Uuid(device_id),
                Value::Text(owner.to_string()),
                Value::Text(name.into()),
                Value::Text(endpoint_id.into()),
                Value::Integer(1),
                Value::Null,
            ]
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "initial Node Device conflicts with persisted setup",
        ));
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
        return Ok(());
    }
    policies
        .set_prevalidated(expected)
        .await
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use idp_model::model::Id;

    use crate::AppConfig;

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
        use device_service::{DeviceRepo, replica::DbDeviceRepo};

        let root = std::env::temp_dir().join(format!("device-open-{}", Id::now_v7()));
        let config = AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            ..Default::default()
        };
        {
            let database = super::open_local_device_database(&root)
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
            let database = super::open_device_database(&config)
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
            idp_model::model::Id::now_v7()
        ));
        let config: crate::AppConfig = serde_json::from_value(serde_json::json!({
            "data_dir": root,
            "device_database": {"endpoint": "http://127.0.0.1:1", "deadline_ms": 100}
        }))
        .expect("remote startup configuration");
        assert!(
            super::open_device_database(&config).await.is_err(),
            "remote configuration cannot open a local owner"
        );
        assert!(
            crate::device_client(&config).await.is_err(),
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
