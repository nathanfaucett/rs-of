use crate::AppConfig;
use db::{Database, Row, SqlTranslator, Value};
use device_service::replica::{DbSelectionPolicyRepo, SelectionPolicy};
use idp_model::model::Id;
use std::{fs, io, path::Path, sync::Arc};
pub async fn open_device_database(config: &AppConfig) -> io::Result<Arc<Database>> {
    if config.device_database.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local Device owner and remote SQL are mutually exclusive",
        ));
    }
    let directory = Path::new(&config.data_dir);
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
