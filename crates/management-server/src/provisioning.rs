use std::{fs, io, path::Path, sync::Arc};

use db::{Database, Row, SqlTranslator, Value};
use idp_model::{
    contract::IdentityAction,
    model::{Id, Role},
};
use management_service::{
    ManagementService,
    replica::{DbPermissionRepo, DbRoleRepo, DbSelectionPolicyRepo, SelectionPolicy},
};

use crate::AppConfig;

pub async fn identity_client(config: &AppConfig) -> io::Result<db::Client> {
    if let Some(remote) = &config.identity_database {
        let client = remote.connect().await?;
        idp_model::replica::validate_identity(&client)
            .await
            .map_err(io::Error::other)?;
        return Ok(client);
    }
    Ok(open_database(config).await?.client())
}

pub async fn open_database(config: &AppConfig) -> io::Result<Arc<Database>> {
    if config.identity_database.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "remote identity consumers cannot open or migrate a private identity database",
        ));
    }
    let engine = open_device_database(config).await?;
    idp_model::replica::up_policy(&engine)
        .await
        .map_err(io::Error::other)?;
    Ok(engine)
}

pub async fn open_device_database(config: &AppConfig) -> io::Result<Arc<Database>> {
    let directory = Path::new(&config.data_dir);
    fs::create_dir_all(directory)?;
    let engine =
        Arc::new(Database::open(directory.join("management.redb")).map_err(io::Error::other)?);
    management_service::replica::up_devices(&engine)
        .await
        .map_err(io::Error::other)?;
    Ok(engine)
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

pub async fn provision_initial_device(
    engine: Arc<Database>,
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
    let mut transaction = engine
        .client()
        .transaction()
        .await
        .map_err(io::Error::other)?;
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

    let policies = DbSelectionPolicyRepo::new(Arc::clone(&engine));
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
    use std::fs;

    use idp_model::{contract::IdentityAction, model::Id};

    use crate::provision_initial_administrator;

    #[tokio::test]
    async fn initial_device_rejects_noncanonical_endpoint_without_seeding() {
        use management_service::{DeviceRepo, replica::DbDeviceRepo};
        let engine = std::sync::Arc::new(db::Database::in_memory());
        management_service::replica::up(&engine)
            .await
            .expect("owner schema");
        assert!(
            super::provision_initial_device(
                engine.clone(),
                Id::now_v7(),
                Id::now_v7(),
                "not-an-endpoint",
                "node"
            )
            .await
            .is_err()
        );
        assert!(!DbDeviceRepo::new(engine).has_any().await.expect("no seed"));
    }

    #[tokio::test]
    async fn initial_device_policy_retries_with_empty_selection_without_resurrecting_restrictions()
    {
        use db::Database;
        use management_service::replica::DbSelectionPolicyRepo;
        use std::sync::Arc;
        let root = std::env::temp_dir().join(format!("initial-device-policy-{}", Id::now_v7()));
        fs::create_dir_all(&root).expect("owner root");
        let engine = Arc::new(Database::open(root.join("management.redb")).expect("open owner"));
        management_service::replica::up(&engine)
            .await
            .expect("initialize owner schema");
        let device = Id::now_v7();
        let owner = Id::now_v7();
        let endpoint = iroh::SecretKey::generate().public().to_string();
        let name = "node";
        super::provision_initial_device(engine.clone(), device, owner, &endpoint, name)
            .await
            .expect("persist local Device policy");
        super::provision_initial_device(engine.clone(), device, owner, &endpoint, name)
            .await
            .expect("exact interrupted retry");
        {
            let policies = DbSelectionPolicyRepo::new(engine.clone());
            let policy = policies
                .get(device, &owner.to_string(), Id::nil())
                .await
                .expect("read initial policy")
                .expect("policy exists");
            assert!(policy.admin_allowed);
            assert!(
                policies
                    .selected_resources_for_device(device)
                    .await
                    .expect("initial selection")
                    .is_empty()
            );
            policies
                .set_admin_allowed_prevalidated(device, &owner.to_string(), false)
                .await
                .expect("learn local restriction");
        }
        assert!(
            super::provision_initial_device(engine.clone(), device, owner, &endpoint, name)
                .await
                .is_err(),
            "setup retry cannot undo a learned restriction"
        );
        assert!(
            super::provision_initial_device(engine.clone(), device, Id::now_v7(), &endpoint, name)
                .await
                .is_err(),
            "retry cannot reassign an existing Device policy"
        );
        drop(engine);
        fs::remove_dir_all(root).expect("remove Device policy fixture");
    }

    #[tokio::test]
    async fn owner_provisioning_is_retry_safe_in_its_own_data_root() {
        let root = std::env::temp_dir().join(format!("management-owner-{}", Id::now_v7()));

        let user_id = Id::now_v7();
        let role_id = Id::now_v7();
        let permissions = [
            (Id::now_v7(), IdentityAction::DevicePairingRead),
            (Id::now_v7(), IdentityAction::DevicePairingUpdate),
        ];

        let engine = super::open_database(&crate::AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            ..crate::AppConfig::default()
        })
        .await
        .expect("open and initialize owner once");
        let first = provision_initial_administrator(engine.clone(), user_id, role_id, &permissions)
            .await
            .expect("provision initial administrator");
        let retry = provision_initial_administrator(engine.clone(), user_id, role_id, &permissions)
            .await
            .expect("retry initial administrator provisioning");

        assert_eq!(first.id, role_id);
        assert_eq!(retry.id, first.id);
        assert!(root.join("management.redb").is_file());
        drop(engine);
        fs::remove_dir_all(root).expect("remove temporary Management data");
    }
}
