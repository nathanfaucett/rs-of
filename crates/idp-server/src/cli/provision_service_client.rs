use crate::cli::provision_service_client::run::std::fs::self;
use crate::cli::provision_service_client::run::std::fs::OpenOptions;
use crate::cli::provision_service_client::run::std::io::self;
use crate::cli::provision_service_client::run::std::io::Write;
use crate::cli::provision_service_client::run::std::crate::cli::provision_service_client::run::path::Path;
use crate::cli::provision_service_client::run::std::crate::cli::provision_service_client::run::sync::Arc;

use crate::cli::provision_service_client::run::ofdb_sql::database::Database;
use crate::cli::provision_service_client::run::run::serde::Serialize;
use idp_service::{
    replica::{
        application_repo::DbApplicationRepo, client_repo::DbClientRepo, key_repo::DbKeyRepo,
    },
    repo::{
        appliccation_repo::ApplicationRepo, client_repo::ClientRepo, key_repo::KeyRepo,
        key_service::KeyService, private_key_keyring_repo::PrivateKeyKeyringRepo,
        private_key_repo::PrivateKeyRepo,
    },
    util::generate_random_string,
};
use of_model::idp::contract::{
    application_registration::ApplicationRegistration, client_profile::ClientProfile,
    client_registration::ClientRegistration, client_type::ClientType, entity_type::EntityType,
    grant_type::GrantType, token_endpoint_auth_method::TokenEndpointAuthMethod,
};

use crate::config::app_config::AppConfig;

#[derive(Serialize)]
struct ClientCredentials {
    client_id: String,
    client_secret: String,
}

pub async fn run(
    app_config: &AppConfig,
    application_uri: &str,
    client_name: &str,
    audiences: Vec<String>,
    scopes: Vec<String>,
    credentials_path: &Path,
) -> crate::cli::provision_service_client::run::io::Result<()> {
    validate_values(application_uri, client_name, &audiences, &scopes)?;
    #[cfg(not(unix))]
    return Err(crate::cli::provision_service_client::run::io::Error::new(
        crate::cli::provision_service_client::run::io::ErrorKind::Unsupported,
        "service-client credential files require Unix owner-only permissions",
    ));

    let database_path =
        crate::cli::provision_service_client::run::Path::new(&app_config.data_dir).join("idp.redb");
    if !database_path.is_file() {
        return Err(crate::cli::provision_service_client::run::io::Error::new(
            crate::cli::provision_service_client::run::io::ErrorKind::NotFound,
            format!("IdP database does not exist: {}", database_path.display()),
        ));
    }
    if credentials_path.exists() {
        return Err(crate::cli::provision_service_client::run::io::Error::new(
            crate::cli::provision_service_client::run::io::ErrorKind::AlreadyExists,
            "credential output file already exists",
        ));
    }

    let engine = crate::cli::provision_service_client::run::Arc::new(
        crate::cli::provision_service_client::run::Database::open(database_path)
            .map_err(crate::cli::provision_service_client::run::io::Error::other)?,
    );

    let key_service = crate::cli::provision_service_client::run::Arc::new(
        crate::cli::provision_service_client::run::KeyService::new(
            crate::cli::provision_service_client::run::DbKeyRepo::new(engine.client()),
            crate::cli::provision_service_client::run::PrivateKeyKeyringRepo::new(
                &app_config.oauth2.issuer,
            )
            .map_err(crate::cli::provision_service_client::run::io::Error::other)?,
            app_config.key_namespace.clone(),
        ),
    );
    let client_repo = crate::cli::provision_service_client::run::DbClientRepo::new(
        engine.client(),
        crate::cli::provision_service_client::run::Arc::clone(&key_service),
    );
    provision(
        &crate::cli::provision_service_client::run::DbApplicationRepo::new(engine.client()),
        &client_repo,
        &key_service,
        application_uri,
        client_name,
        audiences,
        scopes,
        credentials_path,
    )
    .await
}

async fn provision<A, C, K, P>(
    application_repo: &A,
    client_repo: &C,
    key_service: &KeyService<K, P>,
    application_uri: &str,
    client_name: &str,
    audiences: Vec<String>,
    scopes: Vec<String>,
    credentials_path: &Path,
) -> crate::cli::provision_service_client::run::io::Result<()>
where
    A: ApplicationRepo,
    C: ClientRepo,
    K: KeyRepo,
    P: PrivateKeyRepo,
{
    let application = application_repo
        .find_by_uri(application_uri)
        .await
        .map_err(crate::cli::provision_service_client::run::io::Error::other)?
        .ok_or_else(|| {
            crate::cli::provision_service_client::run::io::Error::new(
                crate::cli::provision_service_client::run::io::ErrorKind::NotFound,
                "application not found",
            )
        })?;

    let client_id = generate_random_string::<32>();
    let client_secret = generate_random_string::<64>();
    if client_repo
        .find_client_by_client_id(&client_id)
        .await
        .map_err(crate::cli::provision_service_client::run::io::Error::other)?
        .is_some()
    {
        return Err(crate::cli::provision_service_client::run::io::Error::new(
            crate::cli::provision_service_client::run::io::ErrorKind::AlreadyExists,
            "generated client ID already exists; retry provisioning",
        ));
    }

    let credentials = ClientCredentials {
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
    };
    let credential_bytes =
        crate::cli::provision_service_client::run::serde_json::to_vec(&credentials)
            .map_err(crate::cli::provision_service_client::run::io::Error::other)?;
    let mut options = crate::cli::provision_service_client::run::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use crate::cli::provision_service_client::run::run::std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut credentials_file = options.open(credentials_path)?;
    if let Err(error) = credentials_file
        .write_all(&credential_bytes)
        .and_then(|()| credentials_file.sync_all())
    {
        drop(credentials_file);
        crate::cli::provision_service_client::run::fs::remove_file(credentials_path)?;
        return Err(error);
    }
    drop(credentials_file);
    #[cfg(unix)]
    {
        let parent = credentials_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| crate::cli::provision_service_client::run::Path::new("."));
        if let Err(error) = crate::cli::provision_service_client::run::std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
        {
            crate::cli::provision_service_client::run::fs::remove_file(credentials_path)?;
            return Err(error);
        }
    }

    let registration = ClientRegistration {
        application: ApplicationRegistration {
            name: Some(application.name),
            uri: application.uri,
            description: application.description,
        },
        client_id: Some(client_id.clone()),
        client_secret: Some(client_secret),
        client_id_issued_at: None,
        client_secret_expires_at: None,
        client_name: client_name.to_owned(),
        client_uri: None,
        logo_uri: None,
        contacts: crate::cli::provision_service_client::run::Vec::new(),
        terms_of_service_uri: None,
        policy_uri: None,
        client_type: crate::cli::provision_service_client::run::ClientType::Confidential,
        profile: crate::cli::provision_service_client::run::ClientProfile::Web,
        redirect_uris: crate::cli::provision_service_client::run::Vec::new(),
        allowed_grant_types: vec![
            crate::cli::provision_service_client::run::GrantType::ClientCredentials,
        ],
        response_types: crate::cli::provision_service_client::run::Vec::new(),
        allowed_scopes: scopes,
        allowed_audiences: audiences,
        token_endpoint_auth_method:
            crate::cli::provision_service_client::run::TokenEndpointAuthMethod::ClientSecretPost,
        software_statement: None,
        software_id: None,
        software_version: None,
    };

    match client_repo.create_client(registration).await {
        Ok(_) => Ok(()),
        Err(error) => {
            let partial_client = client_repo
                .find_client_by_client_id(&client_id)
                .await
                .map_err(|cleanup_error| {
                    crate::cli::provision_service_client::run::io::Error::other(format!(
                        "provisioning failed ({error}); could not verify client cleanup ({cleanup_error}); credentials retained at {}",
                        credentials_path.display()
                    ))
                })?;
            if let Some(partial_client) = partial_client {
                key_service
                    .delete_entity_key_material(crate::cli::provision_service_client::run::EntityType::Client, partial_client.id)
                    .await
                    .map_err(|cleanup_error| {
                        crate::cli::provision_service_client::run::io::Error::other(format!(
                            "provisioning failed ({error}); key cleanup failed ({cleanup_error}); credentials retained at {}",
                            credentials_path.display()
                        ))
                    })?;
                client_repo
                    .delete_client_by_client_id(&client_id)
                    .await
                    .map_err(|cleanup_error| {
                        crate::cli::provision_service_client::run::io::Error::other(format!(
                            "provisioning failed ({error}); client cleanup failed ({cleanup_error}); credentials retained at {}",
                            credentials_path.display()
                        ))
                    })?;
            }
            crate::cli::provision_service_client::run::fs::remove_file(credentials_path)?;
            Err(crate::cli::provision_service_client::run::io::Error::other(
                error,
            ))
        }
    }
}

fn validate_values(
    application_uri: &str,
    client_name: &str,
    audiences: &[String],
    scopes: &[String],
) -> crate::cli::provision_service_client::run::io::Result<()> {
    if application_uri.trim().is_empty()
        || client_name.trim().is_empty()
        || audiences.is_empty()
        || scopes.is_empty()
        || audiences
            .iter()
            .chain(scopes)
            .any(|value| value.trim().is_empty() || value.trim() != value)
        || has_duplicates(audiences)
        || has_duplicates(scopes)
    {
        return Err(crate::cli::provision_service_client::run::io::Error::new(
            crate::cli::provision_service_client::run::io::ErrorKind::InvalidInput,
            "application, client name, unique audiences, and unique scopes are required",
        ));
    }
    Ok(())
}

fn has_duplicates(values: &[String]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
}

#[cfg(test)]
mod tests {
    use crate::cli::provision_service_client::run::std::crate::cli::provision_service_client::run::any::Any;
    use crate::cli::provision_service_client::run::std::crate::cli::provision_service_client::run::collections::HashMap;
    use crate::cli::provision_service_client::run::std::fs;
    use crate::cli::provision_service_client::run::std::io::Read;
    use crate::cli::provision_service_client::run::std::io::Write;
    use crate::cli::provision_service_client::run::std::crate::cli::provision_service_client::run::net::TcpStream;
    use crate::cli::provision_service_client::run::std::crate::cli::provision_service_client::run::os::unix::fs::PermissionsExt;
    use crate::cli::provision_service_client::run::std::sync::Arc;
    use crate::cli::provision_service_client::run::std::sync::Mutex;

    use crate::cli::provision_service_client::run::ofdb_sql::database::Database;
    use idp_service::{oauth2::{config::{OAuth2Config}, jwt::{decode_jwt}, service::{OAuth2Service}}, password_config::{PasswordConfig}, replica::{application_repo::{DbApplicationRepo}, client_repo::{DbClientRepo}, key_repo::{DbKeyRepo}, oauth2_authorization_code_repo::{DbOAuth2AuthorizationCodeRepo}, oauth2_refresh_token_repo::{DbOAuth2RefreshTokenRepo}, oauth2_user_consent_repo::{DbOAuth2UserConsentRepo}, user_repo::{DbUserRepo}}, repo::{appliccation_repo::{ApplicationRepo}, client_repo::{ClientRepo}, error::{RepoResult}, key_repo::{KeyRepo}, key_service::{KeyService}, oauth2_authorization_code_repo::{OAuth2AuthorizationCodeRepo}, private_key_keyring_repo::{PrivateKeyKeyringRepo}, private_key_repo::{PrivateKeyRepo}, user_repo::{UserRepo}}};
    use crate::cli::provision_service_client::run::iroh::Endpoint;
    use crate::cli::provision_service_client::run::iroh::SecretKey;
    use crate::cli::provision_service_client::run::iroh::crate::cli::provision_service_client::run::endpoint::presets;
    use bip32::DerivationPath;
    use key::derived_key::DerivedKey;
    use management_service::local_permissions::LocalPermissions;
    use of_model::contract::{principal_type::{PrincipalType}, standard_claims::{StandardClaims}, token_type::{TokenType}, token_use::{TokenUse}};
    use of_model::idp::contract::{client_credentials_grant_request::{ClientCredentialsGrantRequest}, entity_type::{EntityType}, introspection_request::{IntrospectionRequest}, oauth2_client_auth::{OAuth2ClientAuth}, token_request::{TokenRequest}};

    struct ModifierTolerantTestStore {
        inner: Arc<crate::cli::provision_service_client::run::keyring_core::mock::Store>,
    }

    impl crate::cli::provision_service_client::run::keyring_core::api::CredentialStoreApi
        for ModifierTolerantTestStore
    {
        fn vendor(&self) -> String {
            self.inner.vendor()
        }

        fn id(&self) -> String {
            self.inner.id()
        }

        fn build(
            &self,
            service: &str,
            user: &str,
            _modifiers: Option<&HashMap<&str, &str>>,
        ) -> crate::cli::provision_service_client::run::keyring_core::Result<
            crate::cli::provision_service_client::run::keyring_core::Entry,
        > {
            crate::cli::provision_service_client::run::keyring_core::api::CredentialStoreApi::build(
                &*self.inner,
                service,
                user,
                None,
            )
        }

        fn search(
            &self,
            spec: &HashMap<&str, &str>,
        ) -> crate::cli::provision_service_client::run::keyring_core::Result<
            Vec<crate::cli::provision_service_client::run::keyring_core::Entry>,
        > {
            crate::cli::provision_service_client::run::keyring_core::api::CredentialStoreApi::search(
                &*self.inner,
                spec,
            )
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    fn test_keyring_store()
    -> Arc<crate::cli::provision_service_client::run::keyring_core::CredentialStore> {
        crate::cli::provision_service_client::run::Arc::new(ModifierTolerantTestStore {
            inner: crate::cli::provision_service_client::run::keyring_core::mock::Store::new()
                .expect("create mock credential store"),
        })
    }

    use crate::cli::provision_service_client::run::has_duplicates;

    use crate::cli::provision_service_client::run::provision;

    use crate::cli::provision_service_client::run::validate_values;

    #[derive(Clone)]
    struct TestPrivateKeyRepo {
        keys: Arc<Mutex<HashMap<(String, String), String>>>,
        store_calls: Arc<Mutex<usize>>,
        fail_on_store: Option<usize>,
    }

    impl TestPrivateKeyRepo {
        fn new(fail_on_store: Option<usize>) -> Self {
            Self {
                keys: crate::cli::provision_service_client::run::Arc::new(
                    crate::cli::provision_service_client::run::Mutex::new(
                        crate::cli::provision_service_client::run::HashMap::new(),
                    ),
                ),
                store_calls: crate::cli::provision_service_client::run::Arc::new(
                    crate::cli::provision_service_client::run::Mutex::new(0),
                ),
                fail_on_store,
            }
        }
    }

    impl PrivateKeyRepo for TestPrivateKeyRepo {
        fn load(
            &self,
            namespace: &str,
            derivation_path: &DerivationPath,
        ) -> RepoResult<Option<DerivedKey>> {
            let keys = self.keys.lock().expect("test key map lock");
            Ok(keys
                .get(&(namespace.to_owned(), derivation_path.to_string()))
                .map(|key| {
                    crate::cli::provision_service_client::run::DerivedKey::from_xprv(
                        key.clone(),
                        derivation_path.clone(),
                    )
                })
                .transpose()?)
        }

        fn store(&self, namespace: &str, derived_key: &DerivedKey) -> RepoResult<()> {
            let mut store_calls = self.store_calls.lock().expect("test store count lock");
            *store_calls += 1;
            if self.fail_on_store == Some(*store_calls) {
                return Err(idp_service::repo::error::RepoError::InvalidInput(
                    "injected keyring write failure".to_owned(),
                ));
            }
            self.keys.lock().expect("test key map lock").insert(
                (
                    namespace.to_owned(),
                    derived_key.derivation_path().to_string(),
                ),
                derived_key.to_xprv_string().to_string(),
            );
            Ok(())
        }

        fn delete(&self, namespace: &str, derivation_path: &DerivationPath) -> RepoResult<()> {
            self.keys
                .lock()
                .expect("test key map lock")
                .remove(&(namespace.to_owned(), derivation_path.to_string()));
            Ok(())
        }
    }

    #[test]
    fn service_client_inputs_require_unique_non_empty_audiences_and_scopes() {
        assert!(
            validate_values(
                "app",
                "service",
                &["idp".to_owned()],
                &["idp.token.validate".to_owned()]
            )
            .is_ok()
        );
        assert!(validate_values("app", "service", &[], &["scope".to_owned()]).is_err());
        assert!(
            validate_values(
                "app",
                "service",
                &["audience".to_owned(), "audience".to_owned()],
                &["scope".to_owned()]
            )
            .is_err()
        );
        assert!(
            validate_values(
                "app",
                "service",
                &["audience".to_owned()],
                &["scope".to_owned(), "scope".to_owned()]
            )
            .is_err()
        );
    }

    #[test]
    fn duplicate_check_handles_unique_values() {
        assert!(!has_duplicates(&["one".to_owned(), "two".to_owned()]));
    }

    #[test]
    fn provider_denied_registration_creates_metadata_only_but_node_signer_write_denies() {
        crate::cli::provision_service_client::run::std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                crate::cli::provision_service_client::run::tokio::runtime::Builder::new_current_thread()
                    .build()
                    .expect("build test runtime")
                    .block_on(async {
                        let root = crate::cli::provision_service_client::run::std::env::temp_dir().join(format!(
                            "idp-service-client-metadata-{}-{}",
                            crate::cli::provision_service_client::run::std::process::id(),
                            of_model::id::Id::now_v7()
                        ));
                        crate::cli::provision_service_client::run::fs::create_dir_all(&root).expect("create temporary test directory");
                        let database_path = root.join("idp.redb");
                        let credentials_path = root.join("credentials.json");
                        let engine =
                            crate::cli::provision_service_client::run::Arc::new(crate::cli::provision_service_client::run::Database::open(&database_path).expect("open test database"));
                        of_model::idp::replica::up(&engine)
                            .await
                            .expect("initialize current IdP schema");
                        let applications = crate::cli::provision_service_client::run::DbApplicationRepo::new(engine.client());
                        applications
                            .create_application(
                                "Test application".to_owned(),
                                "https://example.test/app".to_owned(),
                                None,
                            )
                            .await
                            .expect("create canonical application");
                        let private_keys = crate::cli::provision_service_client::run::TestPrivateKeyRepo::new(Some(1));
                        let key_service = crate::cli::provision_service_client::run::Arc::new(crate::cli::provision_service_client::run::KeyService::new(
                            crate::cli::provision_service_client::run::DbKeyRepo::new(engine.client()),
                            private_keys.clone(),
                            "test-namespace".to_owned(),
                        ));
                        let clients = crate::cli::provision_service_client::run::DbClientRepo::new(engine.client(), crate::cli::provision_service_client::run::Arc::clone(&key_service));

                        provision(
                            &applications,
                            &clients,
                            &key_service,
                            "https://example.test/app",
                            "Test service",
                            vec!["https://management.example.test".to_owned()],
                            vec!["management.replication.read".to_owned()],
                            &credentials_path,
                        )
                        .await
                        .expect("Client registration does not require provider writes");
                        let credential_bytes =
                            crate::cli::provision_service_client::run::fs::read(&credentials_path).expect("retained confidential credentials");
                        let credentials: crate::cli::provision_service_client::run::serde_json::Value =
                            crate::cli::provision_service_client::run::serde_json::from_slice(&credential_bytes).expect("credential document");
                        let client = clients
                            .find_client_by_client_id(
                                credentials["client_id"].as_str().expect("Client ID"),
                            )
                            .await
                            .expect("lookup provisioned Client")
                            .expect("durable Client");
                        assert!(!client.client_secret_hash.is_empty());
                        assert!(
                            credentials["client_secret"]
                                .as_str()
                                .is_some_and(|secret| !secret.is_empty())
                        );
                        let root_key = key_service
                            .key_repo()
                            .find_active_entity_root_key(crate::cli::provision_service_client::run::EntityType::Client, client.id)
                            .await
                            .expect("Client root lookup")
                            .expect("actual metadata root");
                        assert_eq!(root_key.entity_type, crate::cli::provision_service_client::run::EntityType::Client);
                        assert_eq!(root_key.entity_id, client.id);
                        assert!(root_key.parent_id.is_none());
                        assert!(root_key.public_jwk.is_none());
                        assert_eq!(*private_keys.store_calls.lock().expect("store count"), 0);
                        assert!(private_keys.keys.lock().expect("private store").is_empty());

                        let binding = idp_service::oauth2::node_signer::NodeSignerBinding {
                            member_id: of_model::id::Id::now_v7(),
                            key_id: of_model::id::Id::now_v7(),
                            endpoint_id: "provider-denied-endpoint".into(),
                        };
                        let error = key_service
                            .prepare_node_signer(&binding)
                            .expect_err("required Node private write still fails closed");
                        assert!(error.to_string().contains("injected keyring write failure"));
                        assert_eq!(*private_keys.store_calls.lock().expect("store count"), 1);
                        assert!(private_keys.keys.lock().expect("private store").is_empty());
                        assert!(
                            crate::cli::provision_service_client::run::fs::read(&credentials_path).expect("credentials remain")
                                == credential_bytes
                        );
                        assert_eq!(
                            key_service
                                .key_repo()
                                .find_active_entity_root_key(crate::cli::provision_service_client::run::EntityType::Client, client.id)
                                .await
                                .expect("Client root remains")
                                .expect("metadata root"),
                            root_key
                        );
                        assert_eq!(
                            clients
                                .find_client_by_client_id(&client.client_id)
                                .await
                                .expect("Client remains")
                                .expect("durable Client"),
                            client
                        );

                        drop(clients);
                        drop(applications);
                        drop(engine);
                        crate::cli::provision_service_client::run::fs::remove_dir_all(root).expect("remove temporary test directory");
                    });
            })
            .expect("spawn test thread")
            .join()
            .expect("test thread did not panic");
    }

    #[test]
    fn live_idp_listener_issues_and_introspects_service_tokens() {
        crate::cli::provision_service_client::run::std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                crate::cli::provision_service_client::run::tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .thread_stack_size(16 * 1024 * 1024)
                    .enable_all()
                    .build()
                    .expect("build test runtime")
                    .block_on(run_live_idp_listener_test());
            })
            .expect("spawn large-stack test thread")
            .join()
            .expect("test thread did not panic");
    }

    async fn run_live_idp_listener_test() {
        let root = crate::cli::provision_service_client::run::std::env::temp_dir().join(format!(
            "idp-live-service-client-{}-{}",
            crate::cli::provision_service_client::run::std::process::id(),
            of_model::id::Id::now_v7()
        ));
        crate::cli::provision_service_client::run::fs::create_dir_all(&root)
            .expect("create temporary test directory");
        let engine = crate::cli::provision_service_client::run::Arc::new(
            crate::cli::provision_service_client::run::Database::open(root.join("idp.redb"))
                .expect("open test database"),
        );
        of_model::idp::replica::up(&engine)
            .await
            .expect("initialize current IdP schema");
        let applications =
            crate::cli::provision_service_client::run::DbApplicationRepo::new(engine.client());
        applications
            .create_application(
                "Test application".to_owned(),
                "https://example.test/app".to_owned(),
                None,
            )
            .await
            .expect("create canonical application");
        let key_service = crate::cli::provision_service_client::run::Arc::new(
            crate::cli::provision_service_client::run::KeyService::new(
                crate::cli::provision_service_client::run::DbKeyRepo::new(engine.client()),
                crate::cli::provision_service_client::run::PrivateKeyKeyringRepo::new_with_store(
                    "idp-live-test",
                    test_keyring_store(),
                ),
                "live-test-namespace".to_owned(),
            ),
        );
        let clients = crate::cli::provision_service_client::run::DbClientRepo::new(
            engine.client(),
            crate::cli::provision_service_client::run::Arc::clone(&key_service),
        );
        let credentials_path = root.join("service-credentials.json");
        provision(
            &applications,
            &clients,
            &key_service,
            "https://example.test/app",
            "management-to-idp",
            vec![
                "https://example.test/app".to_owned(),
                "https://idp.example.test".to_owned(),
                "https://management.example.test".to_owned(),
            ],
            vec!["idp.token.validate".to_owned()],
            &credentials_path,
        )
        .await
        .expect("provision test service client");
        let credentials: crate::cli::provision_service_client::run::serde_json::Value =
            crate::cli::provision_service_client::run::serde_json::from_slice(
                &crate::cli::provision_service_client::run::fs::read(&credentials_path)
                    .expect("read service credentials"),
            )
            .expect("parse service credentials");
        let client_id = credentials["client_id"]
            .as_str()
            .expect("service client ID is a string");
        let client_secret = credentials["client_secret"]
            .as_str()
            .expect("service client secret is a string");
        let application_id = clients
            .find_client_by_client_id(client_id)
            .await
            .expect("read provisioned service client")
            .expect("service client exists")
            .application_id
            .to_string();
        let issuer = "https://idp.example.test";
        let secret_key = crate::cli::provision_service_client::run::SecretKey::generate();
        let endpoint = crate::cli::provision_service_client::run::Endpoint::builder(
            crate::cli::provision_service_client::run::presets::N0,
        )
        .secret_key(secret_key.clone())
        .bind()
        .await
        .expect("bind temporary IdP Iroh endpoint");
        let policy_engine = crate::cli::provision_service_client::run::Arc::new(
            crate::cli::provision_service_client::run::Database::open(root.join("devices.redb"))
                .expect("open local policy store"),
        );
        of_model::idp::replica::up(&policy_engine)
            .await
            .expect("initialize local policy schema");
        device_service::replica::schema::up(&policy_engine)
            .await
            .expect("initialize local device schema");
        let endpoint_id = endpoint.id().to_string();
        let local_signer = idp_service::oauth2::node_signer::NodeSignerBinding {
            member_id: of_model::id::Id::now_v7(),
            key_id: of_model::id::Id::now_v7(),
            endpoint_id: endpoint_id.clone(),
        };
        let public_jwk = key_service
            .prepare_node_signer(&local_signer)
            .expect("prepare independent local Node signer");
        idp_service::replica::replica_signer_repo::DbReplicaSignerRepo::new(engine.client())
            .enroll(
                &of_model::idp::contract::idp_signer::ReplicaMembership {
                    installation_id: issuer.into(),
                    issuer: issuer.into(),
                    member_id: local_signer.member_id,
                    endpoint_id: endpoint_id.clone(),
                    approved_at: 1,
                    revoked_at: None,
                },
                &of_model::idp::contract::idp_signer::IdpSignerRecord {
                    member_id: local_signer.member_id,
                    key_id: local_signer.key_id,
                    issuer: issuer.into(),
                    public_jwk,
                    approved_at: Some(1),
                    revoked_at: None,
                    expires_at: None,
                },
            )
            .await
            .expect("approve matching local signer trust");
        let oauth2_service = crate::cli::provision_service_client::run::Arc::new(
            crate::cli::provision_service_client::run::OAuth2Service::new(
                crate::cli::provision_service_client::run::DbApplicationRepo::new(engine.client()),
                crate::cli::provision_service_client::run::DbClientRepo::new(
                    engine.client(),
                    crate::cli::provision_service_client::run::Arc::clone(&key_service),
                ),
                crate::cli::provision_service_client::run::DbOAuth2AuthorizationCodeRepo::new(
                    engine.client(),
                ),
                crate::cli::provision_service_client::run::DbOAuth2RefreshTokenRepo::new(
                    engine.client(),
                ),
                crate::cli::provision_service_client::run::DbUserRepo::new(
                    engine.client(),
                    crate::cli::provision_service_client::run::PasswordConfig::default(),
                ),
                crate::cli::provision_service_client::run::DbOAuth2UserConsentRepo::new(
                    engine.client(),
                ),
                key_service,
                OAuth2Config {
                    issuer: issuer.to_owned(),
                    local_signer: Some(local_signer.clone()),
                    ..OAuth2Config::default()
                },
            ),
        );
        let refresh_client = oauth2_service
            .client_repo
            .create_client(
                crate::cli::provision_service_client::run::serde_json::from_value(
                    crate::cli::provision_service_client::run::serde_json::json!({
                        "application": { "uri": "https://example.test/app" },
                        "client_id": "live-refresh-client",
                        "client_secret": "live-refresh-secret",
                        "client_name": "Refresh client",
                        "client_type": "confidential",
                        "profile": "web_application",
                        "redirect_uris": ["https://example.test/callback"],
                        "allowed_grant_types": ["authorization_code", "refresh_token"],
                        "response_types": ["code"],
                        "allowed_scopes": ["openid", "setup"],
                        "token_endpoint_auth_method": "client_secret_basic"
                    }),
                )
                .expect("build refresh client registration"),
            )
            .await
            .expect("create live refresh client");
        let refresh_user = oauth2_service
            .user_repo
            .create_user_with_password("refresh-user", "refresh-password")
            .await
            .expect("create refresh user");
        oauth2_service
            .key_service
            .ensure_entity_master_key(
                crate::cli::provision_service_client::run::EntityType::User,
                refresh_user.id,
                "refresh-password",
            )
            .expect("create refresh user master key");
        let (refresh_key, _) = oauth2_service
            .key_service
            .create_key(
                None,
                crate::cli::provision_service_client::run::EntityType::User,
                refresh_user.id,
                true,
                "refresh key".into(),
                None,
            )
            .await
            .expect("create refresh signing key");
        let refresh_code = oauth2_service
            .authorization_code_repo
            .create_authorization_code(
                refresh_client.client_id.clone(),
                refresh_key.id,
                "https://example.test/callback".into(),
                vec!["openid".into(), "setup".into()],
                None,
                None,
                None,
                None,
                (crate::cli::provision_service_client::run::std::time::SystemTime::now()
                    + crate::cli::provision_service_client::run::std::time::Duration::from_secs(
                        300,
                    ))
                .into(),
            )
            .await
            .expect("persist live authorization code");
        let policy = management_service::service::ManagementService::new(
            management_service::replica::permission_repo::DbPermissionRepo::new(
                policy_engine.client(),
            ),
            management_service::replica::role_repo::DbRoleRepo::new(policy_engine.client()),
        );
        let listener =
            crate::cli::provision_service_client::run::tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind IdP HTTP listener");
        let address = listener.local_addr().expect("read IdP HTTP address");
        let state = crate::router::state::RouterState::new(
            issuer,
            "https://example.test/app",
            engine.client(),
            oauth2_service,
            None,
            crate::cli::provision_service_client::run::Arc::new(
                device_service::device_identity::DeviceIdentity::new(endpoint.clone(), secret_key)
                    .expect("matching endpoint identity key"),
            ),
        )
        .with_local_permissions(
            crate::cli::provision_service_client::run::LocalPermissions::new(
                policy_engine.client(),
            ),
        );
        let app = crate::router::openapi_router::openapi_router(state.clone(), "/")
            .split_for_parts()
            .0;
        let server = crate::cli::provision_service_client::run::tokio::spawn(async move {
            crate::cli::provision_service_client::run::axum::serve(listener, app)
                .await
                .expect("serve IdP test router");
        });
        let signer_listener =
            crate::cli::provision_service_client::run::tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind signer authorization listener");
        let signer_address = signer_listener
            .local_addr()
            .expect("read signer authorization address");
        let signer_state = state.clone().with_service_audience("live-refresh-client");
        let signer_app = crate::router::openapi_router::openapi_router(signer_state, "/")
            .split_for_parts()
            .0;
        let signer_server = crate::cli::provision_service_client::run::tokio::spawn(async move {
            crate::cli::provision_service_client::run::axum::serve(signer_listener, signer_app)
                .await
                .expect("serve signer authorization router");
        });

        for (method, path, body) in [
            ("GET", "/oauth2/register/test-client", ""),
            ("POST", "/oauth2/register", "{}"),
            ("PUT", "/oauth2/register/test-client", "{}"),
            ("DELETE", "/oauth2/register/test-client", ""),
        ] {
            let response =
                loopback_http_request(address, method, path, "application/json", body, None);
            assert!(
                response.starts_with("HTTP/1.1 401"),
                "{method} {path}: {response}"
            );
        }
        let removed_sign_route = loopback_http_request(
            address,
            "POST",
            "/device/sign",
            "application/json",
            "{}",
            None,
        );
        assert!(
            removed_sign_route.starts_with("HTTP/1.1 404"),
            "{removed_sign_route}"
        );

        use crate::cli::provision_service_client::run::base64::Engine as _;

        use crate::cli::provision_service_client::run::base64::crate::cli::provision_service_client::run::engine::general_purpose::STANDARD;
        let refresh_auth = format!(
            "Basic {}",
            STANDARD.encode("live-refresh-client:live-refresh-secret")
        );
        let code_form = format!(
            "grant_type=authorization_code&code={}&code_verifier=test-verifier&client_id=live-refresh-client&redirect_uri={}",
            form_encode(&refresh_code.code),
            form_encode("https://example.test/callback"),
        );
        let issued = loopback_http_authorized_request(
            address,
            "POST",
            "/oauth2/token",
            "application/x-www-form-urlencoded",
            &code_form,
            Some(&refresh_auth),
        );
        assert!(issued.starts_with("HTTP/1.1 200"), "{issued}");
        let issued: of_model::contract::token_response::TokenResponse =
            crate::cli::provision_service_client::run::serde_json::from_str(http_response_body(
                &issued,
            ))
            .expect("parse user token response");
        let user_access_token = issued.access_token.0;
        let (_, user_claims) = decode_jwt::<StandardClaims>(&user_access_token)
            .expect("decode issued user access token");
        assert_eq!(user_claims.aud, "live-refresh-client");
        assert_ne!(
            decode_jwt::<StandardClaims>(&user_access_token)
                .expect("user signer header")
                .0
                .kid,
            refresh_key.id.to_string()
        );
        let member_id = of_model::id::Id::now_v7();
        let secondary_endpoint_id =
            crate::cli::provision_service_client::run::SecretKey::generate()
                .public()
                .to_string();
        let peer_binding = idp_service::oauth2::node_signer::NodeSignerBinding {
            member_id,
            key_id: of_model::id::Id::now_v7(),
            endpoint_id: secondary_endpoint_id.clone(),
        };
        let signer_jwk = state
            .oauth2_service
            .key_service
            .prepare_node_signer(&peer_binding)
            .expect("prepare independent peer signer");
        let enrollment = crate::cli::provision_service_client::run::serde_json::json!({
            "memberId": member_id,
            "endpointId": secondary_endpoint_id,
            "publicJwk": signer_jwk,
        })
        .to_string();
        let wrong_audience_enrollment = loopback_http_request(
            address,
            "POST",
            "/replica-signers",
            "application/json",
            &enrollment,
            Some(&user_access_token),
        );
        assert!(
            wrong_audience_enrollment.starts_with("HTTP/1.1 403"),
            "{wrong_audience_enrollment}"
        );
        let signer_enrollment_denied = loopback_http_request(
            signer_address,
            "POST",
            "/replica-signers",
            "application/json",
            &enrollment,
            Some(&user_access_token),
        );
        assert!(
            signer_enrollment_denied.starts_with("HTTP/1.1 403"),
            "{signer_enrollment_denied}"
        );
        let signer_rows = engine
            .client()
            .translate_and_execute(
                "SELECT id FROM idp_replica_signers",
                &crate::cli::provision_service_client::run::ofdb_sql::SqlTranslator,
            )
            .await
            .expect("check rejected enrollment did not create signer state");
        assert_eq!(
            signer_rows[0].rows.len(),
            1,
            "denied enrollment leaves only local signer"
        );
        let membership_rows = engine
            .client()
            .translate_and_execute(
                "SELECT id FROM idp_replica_members",
                &crate::cli::provision_service_client::run::ofdb_sql::SqlTranslator,
            )
            .await
            .expect("check rejected enrollment did not create membership state");
        assert_eq!(
            membership_rows[0].rows.len(),
            1,
            "denied enrollment leaves only local membership"
        );

        use of_model::idp::contract::permission_evaluation::IdentityAction;
        let admin_role = policy
            .provision_initial_administrator(
                refresh_user.id,
                of_model::id::Id::now_v7(),
                &[
                    crate::cli::provision_service_client::run::IdentityAction::DevicePairingRead,
                    crate::cli::provision_service_client::run::IdentityAction::DevicePairingUpdate,
                    crate::cli::provision_service_client::run::IdentityAction::ReplicaSignersEnroll,
                    crate::cli::provision_service_client::run::IdentityAction::ReplicaSignersRotate,
                    crate::cli::provision_service_client::run::IdentityAction::ReplicaSignersRevoke,
                ]
                .map(|action| (of_model::id::Id::now_v7(), action)),
            )
            .await
            .expect("grant local signer administration permissions");
        let approved_enrollment = loopback_http_request(
            signer_address,
            "POST",
            "/replica-signers",
            "application/json",
            &enrollment,
            Some(&user_access_token),
        );
        assert!(
            approved_enrollment.starts_with("HTTP/1.1 201"),
            "{approved_enrollment}"
        );
        let enrollment_retry = loopback_http_request(
            signer_address,
            "POST",
            "/replica-signers",
            "application/json",
            &enrollment,
            Some(&user_access_token),
        );
        assert!(
            enrollment_retry.starts_with("HTTP/1.1 200"),
            "{enrollment_retry}"
        );
        let conflicting_enrollment = crate::cli::provision_service_client::run::serde_json::json!({
            "memberId": member_id,
            "endpointId": "different-endpoint",
            "publicJwk": signer_jwk,
        })
        .to_string();
        let conflict = loopback_http_request(
            signer_address,
            "POST",
            "/replica-signers",
            "application/json",
            &conflicting_enrollment,
            Some(&user_access_token),
        );
        assert!(conflict.starts_with("HTTP/1.1 400"), "{conflict}");

        let replacement_binding = idp_service::oauth2::node_signer::NodeSignerBinding {
            key_id: of_model::id::Id::now_v7(),
            ..peer_binding
        };
        let replacement_jwk = state
            .oauth2_service
            .key_service
            .prepare_node_signer(&replacement_binding)
            .expect("prepare independent replacement signer");
        let replacement_body = crate::cli::provision_service_client::run::serde_json::json!({ "publicJwk": replacement_jwk }).to_string();
        let rotated_signer = loopback_http_request(
            signer_address,
            "PUT",
            &format!("/replica-signers/{member_id}"),
            "application/json",
            &replacement_body,
            Some(&user_access_token),
        );
        assert!(
            rotated_signer.starts_with("HTTP/1.1 200"),
            "{rotated_signer}"
        );
        let revoked_signer = loopback_http_request(
            signer_address,
            "DELETE",
            &format!("/replica-signers/{member_id}"),
            "application/json",
            "",
            Some(&user_access_token),
        );
        assert!(
            revoked_signer.starts_with("HTTP/1.1 200"),
            "{revoked_signer}"
        );
        let active_signers = engine
            .client()
            .translate_and_execute(
                "SELECT key_id FROM idp_replica_signers WHERE active_member_id IS NOT NULL",
                &crate::cli::provision_service_client::run::ofdb_sql::SqlTranslator,
            )
            .await
            .expect("check revocation removed active signer binding");
        assert_eq!(
            active_signers[0].rows.len(),
            1,
            "local issuer signer remains active"
        );
        assert_eq!(
            engine
                .client()
                .translate_and_execute(
                    "SELECT id FROM idp_replica_members",
                    &crate::cli::provision_service_client::run::ofdb_sql::SqlTranslator
                )
                .await
                .expect("count memberships")[0]
                .rows
                .len(),
            2
        );

        let registration = crate::cli::provision_service_client::run::serde_json::to_string(
            &of_model::idp::contract::client_registration::ClientRegistration::from(
                refresh_client.clone(),
            ),
        )
        .expect("serialize valid registration body");
        policy
            .delete_role_by_id(admin_role.application_id, admin_role.id)
            .await
            .expect("revoke local administration grants");
        for (method, path, body) in [
            ("GET", "/oauth2/register/live-refresh-client", ""),
            ("POST", "/oauth2/register", registration.as_str()),
            (
                "PUT",
                "/oauth2/register/live-refresh-client",
                registration.as_str(),
            ),
            ("DELETE", "/oauth2/register/live-refresh-client", ""),
        ] {
            let denied = loopback_http_request(
                address,
                method,
                path,
                "application/json",
                body,
                Some(&user_access_token),
            );
            assert!(
                denied.starts_with("HTTP/1.1 403"),
                "{method} {path}: {denied}"
            );
        }
        assert!(
            clients
                .find_client_by_client_id("live-refresh-client")
                .await
                .expect("read unchanged client")
                .is_some()
        );
        let refresh = issued.refresh_token.expect("issued refresh token").0;
        let refresh_form = format!(
            "grant_type=refresh_token&refresh_token={}",
            form_encode(&refresh)
        );
        let wrong_auth = format!(
            "Basic {}",
            STANDARD.encode("live-refresh-client:wrong-secret")
        );
        let wrong_client_auth = format!(
            "Basic {}",
            STANDARD.encode(format!("{client_id}:{client_secret}"))
        );
        for authorization in [&wrong_auth, &wrong_client_auth] {
            let rejected = loopback_http_authorized_request(
                address,
                "POST",
                "/oauth2/token",
                "application/x-www-form-urlencoded",
                &refresh_form,
                Some(authorization),
            );
            assert!(rejected.starts_with("HTTP/1.1 400"), "{rejected}");
            assert_eq!(
                crate::cli::provision_service_client::run::serde_json::from_str::<
                    crate::cli::provision_service_client::run::serde_json::Value,
                >(http_response_body(&rejected))
                .expect("parse wrong-client rejection")["error"],
                "invalid_client"
            );
        }
        let forged = format!("{refresh}x");
        let forged_form = format!(
            "grant_type=refresh_token&refresh_token={}",
            form_encode(&forged)
        );
        let rejected = loopback_http_authorized_request(
            address,
            "POST",
            "/oauth2/token",
            "application/x-www-form-urlencoded",
            &forged_form,
            Some(&refresh_auth),
        );
        assert!(rejected.starts_with("HTTP/1.1 400"), "{rejected}");
        let rotated = loopback_http_authorized_request(
            address,
            "POST",
            "/oauth2/token",
            "application/x-www-form-urlencoded",
            &refresh_form,
            Some(&refresh_auth),
        );
        assert!(rotated.starts_with("HTTP/1.1 200"), "{rotated}");
        let rotated: of_model::contract::token_response::TokenResponse =
            crate::cli::provision_service_client::run::serde_json::from_str(http_response_body(
                &rotated,
            ))
            .expect("parse rotated token response");
        let replacement = rotated.refresh_token.expect("rotated refresh token").0;
        assert_ne!(refresh, replacement);
        let reuse = loopback_http_authorized_request(
            address,
            "POST",
            "/oauth2/token",
            "application/x-www-form-urlencoded",
            &refresh_form,
            Some(&refresh_auth),
        );
        assert!(reuse.starts_with("HTTP/1.1 400"), "{reuse}");
        assert_eq!(
            crate::cli::provision_service_client::run::serde_json::from_str::<
                crate::cli::provision_service_client::run::serde_json::Value,
            >(http_response_body(&reuse))
            .expect("parse refresh reuse rejection")["error"],
            "invalid_grant"
        );
        let revoke_form = format!(
            "token={}&token_type_hint=refresh_token",
            form_encode(&replacement)
        );
        for authorization in [
            None,
            Some(wrong_auth.as_str()),
            Some(wrong_client_auth.as_str()),
        ] {
            let denied = loopback_http_authorized_request(
                address,
                "POST",
                "/oauth2/revoke",
                "application/x-www-form-urlencoded",
                &revoke_form,
                authorization,
            );
            assert!(denied.starts_with("HTTP/1.1 400"), "{denied}");
            assert_eq!(
                crate::cli::provision_service_client::run::serde_json::from_str::<
                    crate::cli::provision_service_client::run::serde_json::Value,
                >(http_response_body(&denied))
                .expect("parse revoke authentication rejection")["error"],
                "invalid_client"
            );
        }
        for _ in 0..2 {
            let revoked = loopback_http_authorized_request(
                address,
                "POST",
                "/oauth2/revoke",
                "application/x-www-form-urlencoded",
                &revoke_form,
                Some(&refresh_auth),
            );
            assert!(revoked.starts_with("HTTP/1.1 200"), "{revoked}");
        }
        let replacement_form = format!(
            "grant_type=refresh_token&refresh_token={}",
            form_encode(&replacement)
        );
        let revoked = loopback_http_authorized_request(
            address,
            "POST",
            "/oauth2/token",
            "application/x-www-form-urlencoded",
            &replacement_form,
            Some(&refresh_auth),
        );
        assert!(revoked.starts_with("HTTP/1.1 400"), "{revoked}");
        assert_eq!(
            crate::cli::provision_service_client::run::serde_json::from_str::<
                crate::cli::provision_service_client::run::serde_json::Value,
            >(http_response_body(&revoked))
            .expect("parse revoked refresh rejection")["error"],
            "invalid_grant"
        );

        let form = format!(
            "grant_type=client_credentials&client_id={}&client_secret={}&scope=idp.token.validate&audience={}",
            form_encode(client_id),
            form_encode(client_secret),
            form_encode("https://example.test/app"),
        );
        let wrong_secret_form = format!(
            "grant_type=client_credentials&client_id={}&client_secret=wrong-secret&scope=idp.token.validate&audience={}",
            form_encode(client_id),
            form_encode("https://example.test/app"),
        );
        let rejected_token = loopback_http_request(
            address,
            "POST",
            "/oauth2/token",
            "application/x-www-form-urlencoded",
            &wrong_secret_form,
            None,
        );
        assert!(
            rejected_token.starts_with("HTTP/1.1 400"),
            "{rejected_token}"
        );
        let rejected_body: crate::cli::provision_service_client::run::serde_json::Value =
            crate::cli::provision_service_client::run::serde_json::from_str(http_response_body(
                &rejected_token,
            ))
            .expect("parse rejected client authentication response");
        assert_eq!(rejected_body["error"], "invalid_client");

        let token_response = loopback_http_request(
            address,
            "POST",
            "/oauth2/token",
            "application/x-www-form-urlencoded",
            &form,
            None,
        );
        assert!(
            token_response.starts_with("HTTP/1.1 200"),
            "{token_response}"
        );
        let token_body = http_response_body(&token_response);
        let token: crate::cli::provision_service_client::run::serde_json::Value =
            crate::cli::provision_service_client::run::serde_json::from_str(token_body)
                .expect("parse live token response");
        assert!(token["id_token"].is_null());
        assert!(token["refresh_token"].is_null());
        let access_token = token["access_token"]
            .as_str()
            .expect("access token is a string");
        let admin_response = loopback_http_request(
            address,
            "GET",
            "/applications",
            "application/json",
            "",
            Some(access_token),
        );
        assert!(
            admin_response.starts_with("HTTP/1.1 401"),
            "service token must not access user administration: {admin_response}"
        );

        let introspection_body = crate::cli::provision_service_client::run::serde_json::to_string(
            &IntrospectionRequest {
                token: access_token.to_owned(),
                token_type_hint: None,
            },
        )
        .expect("serialize introspection request");
        let introspection_response = loopback_http_request(
            address,
            "POST",
            "/oauth2/introspect",
            "application/json",
            &introspection_body,
            Some(access_token),
        );
        assert!(
            introspection_response.starts_with("HTTP/1.1 200"),
            "{introspection_response}"
        );
        let introspected: of_model::idp::contract::introspection_response::IntrospectionResponse =
            crate::cli::provision_service_client::run::serde_json::from_str(http_response_body(
                &introspection_response,
            ))
            .expect("parse live introspection response");
        assert_eq!(
            introspected.claims.principal_type,
            crate::cli::provision_service_client::run::PrincipalType::Client
        );
        assert_eq!(introspected.claims.client_id, client_id);
        assert_eq!(introspected.claims.aud, "https://example.test/app");
        assert_eq!(introspected.claims.scope, vec!["idp.token.validate"]);
        assert_eq!(introspected.application_id, application_id);

        signer_server.abort();
        let _ = signer_server.await;
        server.abort();
        let _ = server.await;
        drop(state);
        drop(policy);
        drop(policy_engine);
        endpoint.close().await;
        drop(clients);
        drop(applications);
        drop(engine);
        crate::cli::provision_service_client::run::fs::remove_dir_all(root)
            .expect("remove test database directory");
    }

    fn loopback_http_request(
        address: crate::cli::provision_service_client::run::std::net::SocketAddr,
        method: &str,
        path: &str,
        content_type: &str,
        body: &str,
        bearer: Option<&str>,
    ) -> String {
        let authorization = bearer.map(|token| format!("Bearer {token}"));
        loopback_http_authorized_request(
            address,
            method,
            path,
            content_type,
            body,
            authorization.as_deref(),
        )
    }

    fn loopback_http_authorized_request(
        address: crate::cli::provision_service_client::run::std::net::SocketAddr,
        method: &str,
        path: &str,
        content_type: &str,
        body: &str,
        authorization: Option<&str>,
    ) -> String {
        let mut stream = crate::cli::provision_service_client::run::TcpStream::connect(address)
            .expect("connect to IdP HTTP listener");
        stream
            .set_read_timeout(Some(
                crate::cli::provision_service_client::run::std::time::Duration::from_secs(10),
            ))
            .expect("set HTTP read timeout");
        stream
            .set_write_timeout(Some(
                crate::cli::provision_service_client::run::std::time::Duration::from_secs(10),
            ))
            .expect("set HTTP write timeout");
        let authorization = authorization
            .map(|value| format!("Authorization: {value}\r\n"))
            .unwrap_or_default();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{authorization}Connection: close\r\n\r\n{body}",
            body.len(),
        )
        .expect("write request to IdP listener");
        let mut response = crate::cli::provision_service_client::run::String::new();
        stream
            .read_to_string(&mut response)
            .expect("read response from IdP listener");
        response
    }

    fn http_response_body(response: &str) -> &str {
        response
            .split_once("\r\n\r\n")
            .expect("HTTP response has a body separator")
            .1
    }

    fn form_encode(value: &str) -> String {
        let mut encoded = crate::cli::provision_service_client::run::String::new();
        for byte in value.bytes() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                encoded.push(crate::cli::provision_service_client::run::char::from(byte));
            } else {
                use crate::cli::provision_service_client::run::run::std::fmt::Write as _;
                write!(encoded, "%{byte:02X}").expect("write percent-encoded form value");
            }
        }
        encoded
    }

    #[test]
    fn provisioning_creates_scoped_client_and_private_credentials_file() {
        crate::cli::provision_service_client::run::std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                crate::cli::provision_service_client::run::tokio::runtime::Builder::new_current_thread()
                    .build()
                    .expect("build test runtime")
                    .block_on(async {
                        let root = crate::cli::provision_service_client::run::std::env::temp_dir().join(format!(
                            "idp-service-client-{}-{}",
                            crate::cli::provision_service_client::run::std::process::id(),
                            of_model::id::Id::now_v7()
                        ));
                        crate::cli::provision_service_client::run::fs::create_dir_all(&root).expect("create temporary test directory");
                        let database_path = root.join("idp.redb");
                        let credentials_path = root.join("credentials.json");
                        let engine = crate::cli::provision_service_client::run::Arc::new(
                            crate::cli::provision_service_client::run::Database::open(&database_path).expect("open test database"),
                        );
                        of_model::idp::replica::up(&engine)
                            .await
                            .expect("initialize current IdP schema");

                        let applications = crate::cli::provision_service_client::run::DbApplicationRepo::new(engine.client());
                        applications
                            .create_application(
                                "Test application".to_owned(),
                                "https://example.test/app".to_owned(),
                                None,
                            )
                            .await
                            .expect("create canonical application");
                        let key_service = crate::cli::provision_service_client::run::Arc::new(crate::cli::provision_service_client::run::KeyService::new(
                            crate::cli::provision_service_client::run::DbKeyRepo::new(engine.client()),
                            crate::cli::provision_service_client::run::TestPrivateKeyRepo::new(None),
                            "test-namespace".to_owned(),
                        ));
                        let clients =
                            crate::cli::provision_service_client::run::DbClientRepo::new(engine.client(), crate::cli::provision_service_client::run::Arc::clone(&key_service));

                        provision(
                            &applications,
                            &clients,
                            &key_service,
                            "https://example.test/app",
                            "management-to-idp",
                            vec![
                                "https://idp.example.test".to_owned(),
                                "https://user-api.example.test".to_owned(),
                            ],
                            vec![
                                "idp.token.validate".to_owned(),
                                            ],
                            &credentials_path,
                        )
                        .await
                        .expect("provision service client");

                        let credentials: crate::cli::provision_service_client::run::serde_json::Value = crate::cli::provision_service_client::run::serde_json::from_slice(
                            &crate::cli::provision_service_client::run::fs::read(&credentials_path).expect("read credentials"),
                        )
                        .expect("parse credentials");
                        let client_id = credentials["client_id"]
                            .as_str()
                            .expect("credential client ID is a string");
                        let client_secret = credentials["client_secret"]
                            .as_str()
                            .expect("credential secret is a string");
                        let client = clients
                            .find_client_by_client_id(client_id)
                            .await
                            .expect("read created client")
                            .expect("client exists");
                        assert_ne!(client.client_secret_hash, client_secret);
                        assert!(client.client_secret_hash.starts_with("$argon2id$"));
                        assert_eq!(client.client_name, "management-to-idp");
                        assert_eq!(
                            client.allowed_audiences,
                            vec!["https://idp.example.test", "https://user-api.example.test"]
                        );
                        assert_eq!(
                            client.allowed_scopes,
                            vec!["idp.token.validate"]
                        );
                        assert_eq!(
                            client.allowed_grant_types,
                            vec![of_model::idp::contract::grant_type::GrantType::ClientCredentials]
                        );
                        assert_eq!(
                            crate::cli::provision_service_client::run::fs::metadata(&credentials_path)
                                .expect("read credential file metadata")
                                .permissions()
                                .mode()
                                & 0o777,
                            0o600
                        );

                        assert!(
                            provision(
                                &applications,
                                &clients,
                                &key_service,
                                "https://example.test/app",
                                "Second service",
                                vec!["audience".to_owned()],
                                vec!["scope".to_owned()],
                                &credentials_path,
                            )
                            .await
                            .is_err()
                        );
                        assert_eq!(
                            crate::cli::provision_service_client::run::fs::read(&credentials_path)
                                .expect("existing credentials stay unchanged"),
                            crate::cli::provision_service_client::run::serde_json::to_vec(&credentials).expect("serialize credentials")
                        );

                        let first_client_id = client_id.to_owned();
                        let registrations = [
                            (
                                "storage-to-idp",
                                vec![
                                    "https://idp.example.test".to_owned(),
                                    "https://user-api.example.test".to_owned(),
                                ],
                                vec![
                                    "idp.token.validate".to_owned(),
                                                                        ],
                                "storage-idp.json",
                            ),
                            (
                                "storage-to-management",
                                vec!["https://management.example.test".to_owned()],
                                vec![
                                    "management.replication.read".to_owned(),
                                    "management.replication.admit".to_owned(),
                                ],
                                "storage-management.json",
                            ),
                        ];
                        for (name, audiences, scopes, file_name) in registrations {
                            let service_credentials_path = root.join(file_name);
                            provision(
                                &applications,
                                &clients,
                                &key_service,
                                "https://example.test/app",
                                name,
                                audiences.clone(),
                                scopes.clone(),
                                &service_credentials_path,
                            )
                            .await
                            .expect("provision distinct service client");
                            let service_credentials: crate::cli::provision_service_client::run::serde_json::Value = crate::cli::provision_service_client::run::serde_json::from_slice(
                                &crate::cli::provision_service_client::run::fs::read(&service_credentials_path)
                                    .expect("read service credentials"),
                            )
                            .expect("parse service credentials");
                            let service_client_id = service_credentials["client_id"]
                                .as_str()
                                .expect("service client ID is a string");
                            let service_client = clients
                                .find_client_by_client_id(service_client_id)
                                .await
                                .expect("read service client")
                                .expect("service client exists");
                            assert_ne!(service_client.client_id, first_client_id);
                            assert_eq!(service_client.client_name, name);
                            assert_eq!(service_client.allowed_audiences, audiences);
                            assert_eq!(service_client.allowed_scopes, scopes);
                            assert_eq!(
                                service_client.allowed_grant_types,
                                vec![of_model::idp::contract::grant_type::GrantType::ClientCredentials]
                            );
                        }

                        let local_signer = idp_service::oauth2::node_signer::NodeSignerBinding {
                            member_id: of_model::id::Id::now_v7(),
                            key_id: of_model::id::Id::now_v7(),
                            endpoint_id: "test-endpoint".to_owned(),
                        };
                        let public_jwk = key_service
                            .prepare_node_signer(&local_signer)
                            .expect("prepare local Node signer");
                        idp_service::replica::replica_signer_repo::DbReplicaSignerRepo::new(engine.client())
                            .enroll(
                                &of_model::idp::contract::idp_signer::ReplicaMembership {
                                    installation_id: "https://idp.example.test".into(),
                                    issuer: "https://idp.example.test".into(),
                                    member_id: local_signer.member_id,
                                    endpoint_id: local_signer.endpoint_id.clone(),
                                    approved_at: 1,
                                    revoked_at: None,
                                },
                                &of_model::idp::contract::idp_signer::IdpSignerRecord {
                                    member_id: local_signer.member_id,
                                    key_id: local_signer.key_id,
                                    issuer: "https://idp.example.test".into(),
                                    public_jwk,
                                    approved_at: Some(1),
                                    revoked_at: None,
                                    expires_at: None,
                                },
                            )
                            .await
                            .expect("approve local signer trust");
                        let oauth2_service = crate::cli::provision_service_client::run::OAuth2Service::new(
                            crate::cli::provision_service_client::run::DbApplicationRepo::new(engine.client()),
                            crate::cli::provision_service_client::run::DbClientRepo::new(engine.client(), crate::cli::provision_service_client::run::Arc::clone(&key_service)),
                            crate::cli::provision_service_client::run::DbOAuth2AuthorizationCodeRepo::new(engine.client()),
                            crate::cli::provision_service_client::run::DbOAuth2RefreshTokenRepo::new(engine.client()),
                            crate::cli::provision_service_client::run::DbUserRepo::new(engine.client(), crate::cli::provision_service_client::run::PasswordConfig::default()),
                            crate::cli::provision_service_client::run::DbOAuth2UserConsentRepo::new(engine.client()),
                            crate::cli::provision_service_client::run::Arc::clone(&key_service),
                            OAuth2Config {
                                issuer: "https://idp.example.test".to_owned(),
                                local_signer: Some(local_signer),
                                ..OAuth2Config::default()
                            },
                        );
                        for (file_name, audience, scope) in [
                            (
                                "credentials.json",
                                "https://idp.example.test",
                                "idp.token.validate",
                            ),
                            (
                                "storage-idp.json",
                                "https://idp.example.test",
                                "idp.token.validate",
                            ),
                            (
                                "storage-management.json",
                                "https://management.example.test",
                                "management.replication.read management.replication.admit",
                            ),
                        ] {
                            let service_credentials: crate::cli::provision_service_client::run::serde_json::Value = crate::cli::provision_service_client::run::serde_json::from_slice(
                                &crate::cli::provision_service_client::run::fs::read(root.join(file_name)).expect("read provisioned secrets"),
                            )
                            .expect("parse provisioned secrets");
                            let client_id = service_credentials["client_id"]
                                .as_str()
                                .expect("credential client ID is a string")
                                .to_owned();
                            let client_secret = service_credentials["client_secret"]
                                .as_str()
                                .expect("credential secret is a string")
                                .to_owned();
                            let response = oauth2_service
                                .token(
                                    crate::cli::provision_service_client::run::TokenRequest::ClientCredentials(
                                        ClientCredentialsGrantRequest {
                                            client_id: client_id.clone(),
                                            client_secret: client_secret.clone(),
                                            scope: Some(scope.to_owned()),
                                            audience: Some(audience.to_owned()),
                                            resource: None,
                                        },
                                    ),
                                    Some(OAuth2ClientAuth {
                                        client_id: client_id.clone(),
                                        client_secret: Some(client_secret.clone()),
                                        method: of_model::idp::contract::token_endpoint_auth_method::TokenEndpointAuthMethod::ClientSecretPost,
                                    }),
                                )
                                .await
                                .expect("issue scoped service access token");
                            assert_eq!(response.id_token, None);
                            assert_eq!(response.refresh_token, None);
                            let (_, claims) =
                                decode_jwt::<StandardClaims>(&response.access_token.0)
                                    .expect("decode issued service token");
                            assert_eq!(claims.principal_type, crate::cli::provision_service_client::run::PrincipalType::Client);
                            assert_eq!(claims.client_id, client_id);
                            assert_eq!(claims.aud, audience);
                            assert_eq!(
                                claims.scope,
                                scope.split(' ').map(crate::cli::provision_service_client::run::str::to_owned).collect::<Vec<_>>()
                            );
                            assert_eq!(claims.r#type, crate::cli::provision_service_client::run::TokenType::Bearer);
                            assert_eq!(claims.r#use, crate::cli::provision_service_client::run::TokenUse::Access);
                            for (requested_audience, requested_scope) in [
                                ("https://unauthorized.example.test", scope),
                                (audience, "ungranted.scope"),
                            ] {
                                assert!(
                                    oauth2_service
                                        .token(
                                            crate::cli::provision_service_client::run::TokenRequest::ClientCredentials(
                                                ClientCredentialsGrantRequest {
                                                    client_id: client_id.clone(),
                                                    client_secret: client_secret.clone(),
                                                    scope: Some(requested_scope.to_owned()),
                                                    audience: Some(requested_audience.to_owned()),
                                                    resource: None,
                                                },
                                            ),
                                            Some(OAuth2ClientAuth {
                                                client_id: client_id.clone(),
                                                client_secret: Some(client_secret.clone()),
                                                method: of_model::idp::contract::token_endpoint_auth_method::TokenEndpointAuthMethod::ClientSecretPost,
                                            }),
                                        )
                                        .await
                                        .is_err(),
                                    "service token issuance must reject ungranted audience or scope"
                                );
                            }
                        }

                        drop(clients);
                        drop(applications);
                        drop(engine);
                        crate::cli::provision_service_client::run::fs::remove_dir_all(root).expect("remove temporary test directory");
                    });
            })
            .expect("spawn test thread")
            .join()
            .expect("test thread did not panic");
    }
}
