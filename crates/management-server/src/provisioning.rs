use std::{io, sync::Arc};

use db::Database;
use idp_model::{
    contract::IdentityAction,
    model::{Id, Role},
};
use management_service::{
    ManagementService,
    replica::{DbPermissionRepo, DbRoleRepo},
};

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
