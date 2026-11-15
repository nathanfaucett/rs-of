use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use idp_service::{
    oauth2::service::OAuth2Service,
    replica::{
        application_repo::DbApplicationRepo, client_repo::DbClientRepo, key_repo::DbKeyRepo,
        oauth2_authorization_code_repo::DbOAuth2AuthorizationCodeRepo,
        oauth2_refresh_token_repo::DbOAuth2RefreshTokenRepo,
        oauth2_user_consent_repo::DbOAuth2UserConsentRepo, user_repo::DbUserRepo,
    },
    repo::{
        appliccation_repo::ApplicationRepo, client_repo::ClientRepo, key_service::KeyService,
        private_key_keyring_repo::PrivateKeyKeyringRepo, private_key_repo::PrivateKeyRepo,
    },
};
use of_model::id::Id;
use of_model::idp::contract::client_credentials_grant_request::ClientCredentialsGrantRequest;
use of_model::idp::contract::client_registration::ClientRegistration;
use of_model::idp::contract::error_response::ErrorResponse;
use of_model::idp::contract::oauth2_client_auth::OAuth2ClientAuth;
use of_model::idp::contract::token_request::TokenRequest;
use ofdb_sql::Database;

use crate::config::app_config::AppConfig;

/// Opens and migrates local state; retained-state callers must preflight before this creating open.
pub async fn open_local_identity_database(directory: &Path) -> io::Result<Arc<Database>> {
    fs::create_dir_all(directory)?;
    let engine = Arc::new(Database::open(directory.join("idp.redb")).map_err(io::Error::other)?);
    of_model::idp::ofdb::up(&engine)
        .await
        .map_err(io::Error::other)?;
    Ok(engine)
}

fn to_io_error(error: ErrorResponse) -> io::Error {
    io::Error::other(
        error
            .error_description
            .unwrap_or_else(|| format!("IdP provisioning failed: {:?}", error.error)),
    )
}

type NativeOAuth2Service<P> = OAuth2Service<
    DbApplicationRepo,
    DbClientRepo<P>,
    DbOAuth2AuthorizationCodeRepo,
    DbOAuth2RefreshTokenRepo,
    DbUserRepo,
    DbOAuth2UserConsentRepo,
    DbKeyRepo,
    P,
>;

pub struct OwnerProvisioner<P: PrivateKeyRepo + Send + Sync = PrivateKeyKeyringRepo> {
    pub(crate) oauth2: NativeOAuth2Service<P>,
    pub(crate) engine: ofdb_sql::Client,
    pub(crate) database: Option<Arc<Database>>,
    pub(crate) key_namespace: String,
    pub(crate) data_dir: PathBuf,
}

impl OwnerProvisioner {
    pub fn from_database(config: &AppConfig, engine: Arc<Database>) -> io::Result<Self> {
        if config.identity_database.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "injected identity owner and remote SQL are mutually exclusive",
            ));
        }
        Ok(Self::with_database(
            config,
            engine,
            PrivateKeyKeyringRepo::new(&config.oauth2.issuer).map_err(io::Error::other)?,
        ))
    }

    pub async fn open(config: &AppConfig) -> io::Result<Self> {
        Self::open_with_private_keys(
            config,
            PrivateKeyKeyringRepo::new(&config.oauth2.issuer).map_err(io::Error::other)?,
        )
        .await
    }
}

impl<P: PrivateKeyRepo + Send + Sync> OwnerProvisioner<P> {
    pub async fn open_with_private_keys(config: &AppConfig, private_keys: P) -> io::Result<Self> {
        if config.identity_database.is_some() {
            let client = crate::runtime::identity_client(config).await?;
            fs::create_dir_all(&config.data_dir)?;
            return Ok(Self::with_client(config, client, None, private_keys));
        }
        let engine = open_local_identity_database(Path::new(&config.data_dir)).await?;
        Ok(Self::with_database(config, engine, private_keys))
    }

    fn with_database(config: &AppConfig, engine: Arc<Database>, private_keys: P) -> Self {
        Self::with_client(config, engine.client(), Some(engine), private_keys)
    }

    fn with_client(
        config: &AppConfig,
        engine: ofdb_sql::Client,
        database: Option<Arc<Database>>,
        private_keys: P,
    ) -> Self {
        let keys = Arc::new(KeyService::new(
            DbKeyRepo::new(engine.clone()),
            private_keys,
            config.key_namespace.clone(),
        ));
        let oauth2 = OAuth2Service::new(
            DbApplicationRepo::new(engine.clone()),
            DbClientRepo::new(engine.clone(), Arc::clone(&keys)),
            DbOAuth2AuthorizationCodeRepo::new(engine.clone()),
            DbOAuth2RefreshTokenRepo::new(engine.clone()),
            DbUserRepo::new(engine.clone(), config.password.clone()),
            DbOAuth2UserConsentRepo::new(engine.clone()),
            keys,
            config.oauth2.clone(),
        );
        Self {
            oauth2,
            engine,
            database,
            key_namespace: config.key_namespace.clone(),
            data_dir: PathBuf::from(&config.data_dir),
        }
    }

    pub async fn ensure_initial_user(
        &self,
        user_id: Id,
        credential_id: Id,
        name: &str,
        password: &str,
    ) -> io::Result<Id> {
        self.oauth2
            .ensure_initial_user(user_id, credential_id, name, password)
            .await
            .map_err(to_io_error)?;
        Ok(user_id)
    }

    /// Verifies an existing active relationship without provisioning or repairing local state.
    pub async fn verify_infrastructure_client(
        &self,
        mut expected: ClientRegistration,
    ) -> io::Result<()> {
        let id = expected.client_id.as_deref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "existing Client ID is required",
            )
        })?;
        let client = self
            .oauth2
            .client_repo
            .find_client_by_client_id(id)
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "existing Client is missing or inactive",
                )
            })?;
        let secret = expected
            .client_secret
            .take()
            .filter(|secret| !secret.trim().is_empty())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "current Client secret is required",
                )
            })?;

        let application = self
            .oauth2
            .application_repo
            .find_by_id(client.application_id)
            .await
            .map_err(io::Error::other)?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "existing Client Application is missing",
                )
            })?;
        if expected.application.name.is_none() {
            expected.application.name = Some(expected.client_name.clone());
        }
        if expected.client_uri.is_none() {
            expected.client_uri = Some(expected.application.uri.clone());
        }
        let mut current: ClientRegistration = client.into();
        current.application =
            of_model::idp::contract::application_registration::ApplicationRegistration {
                name: Some(application.name),
                uri: application.uri,
                description: application.description,
            };
        if current != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "current Client relationship conflicts with retained intent",
            ));
        }
        let id = expected.client_id.expect("validated existing Client ID");
        self.oauth2
            .token(
                TokenRequest::ClientCredentials(ClientCredentialsGrantRequest {
                    client_id: id.clone(),
                    client_secret: secret.clone(),
                    scope: Some(expected.allowed_scopes.join(" ")),
                    audience: expected.allowed_audiences.first().cloned(),
                    resource: None,
                }),
                Some(OAuth2ClientAuth {
                    client_id: id,
                    client_secret: Some(secret),
                    method: expected.token_endpoint_auth_method,
                }),
            )
            .await
            .map_err(to_io_error)?;
        Ok(())
    }

    pub async fn ensure_infrastructure_client(
        &self,
        request: ClientRegistration,
    ) -> io::Result<ClientRegistration> {
        self.oauth2
            .ensure_infrastructure_client(request)
            .await
            .map_err(to_io_error)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use of_model::id::Id;

    use crate::config::app_config::AppConfig;

    #[tokio::test]
    async fn local_identity_open_initializes_schema_and_reset_preflight_never_replaces_it() {
        let root = std::env::temp_dir().join(format!("identity-open-{}", Id::now_v7()));
        let config = AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert_eq!(
            config
                .require_provisioned_identity()
                .expect_err("reset requires retained root")
                .kind(),
            io::ErrorKind::NotFound
        );
        assert!(!root.exists());
        fs::create_dir(&root).expect("retained empty root");
        assert_eq!(
            config
                .require_provisioned_identity()
                .expect_err("reset requires retained database")
                .kind(),
            io::ErrorKind::NotFound
        );
        assert!(!root.join("idp.redb").exists());
        for _ in 0..2 {
            let database = crate::provisioning::open_local_identity_database(&root)
                .await
                .expect("initialize or reopen identity owner");
            of_model::idp::ofdb::validate_identity(&database.client())
                .await
                .expect("initialized identity schema");
        }
        config
            .require_provisioned_identity()
            .expect("retained identity file");
        fs::remove_file(root.join("idp.redb")).expect("lose retained identity database");
        assert_eq!(
            config
                .require_provisioned_identity()
                .expect_err("reset must not replace lost store")
                .kind(),
            io::ErrorKind::NotFound
        );
        assert!(!root.join("idp.redb").exists());
        fs::remove_dir_all(root).expect("remove identity fixture");
    }
}
