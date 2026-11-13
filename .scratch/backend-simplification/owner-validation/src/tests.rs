use std::sync::Arc;

use embedded_db::MemoryDatabase;
use of_model::{db::Database, idp::model::Id, storage::StorageNamespace};

use crate::{DatabaseCatalog, DatabaseRuntime};

struct Scope(&'static str, Id);
impl StorageNamespace for Scope {
    fn user_sub(&self) -> &str {
        self.0
    }
    fn application_id(&self) -> Id {
        self.1
    }
}

#[tokio::test]
async fn sql_catalog_is_shared_scoped_and_persists_lifecycle_without_catalog_files() {
    let root = tempfile::tempdir().expect("resource root");
    let database = MemoryDatabase::new().await.expect("owner schema");
    let scope = Scope("owner", Id::now_v7());
    database.client().execute_sql(&format!("INSERT INTO namespaces (id, owner_subject, application_id) VALUES (CAST('{}' AS UUID), 'owner', CAST('{}' AS UUID))", Id::now_v7(), scope.1), None).await.expect("namespace already exists in the shared database");
    let catalog = DatabaseCatalog::with_client(root.path().join("first"), database.client())
        .expect("SQL catalog");
    let other = DatabaseCatalog::with_client(root.path().join("other"), database.client())
        .expect("second SQL consumer");
    let resource = catalog
        .create(&scope, Some("O'Reilly".into()))
        .expect("catalog resource");
    assert_eq!(
        other.list(&scope).expect("shared catalog"),
        vec![resource.clone()]
    );
    assert!(
        other
            .register_selected(&Scope("other-owner", scope.1), resource.id)
            .is_err()
    );
    assert_eq!(
        catalog
            .get(&scope, resource.id)
            .expect("original namespace unchanged"),
        Some(resource.clone())
    );
    assert!(
        other
            .list(&Scope("other-owner", scope.1))
            .expect("isolated owner")
            .is_empty()
    );
    let (reserved, pending) = catalog
        .reserve(&scope, "stable-key", Some("pending".into()))
        .expect("reserve");
    assert!(pending);
    assert!(
        other
            .get(&scope, reserved.id)
            .expect("pending hidden")
            .is_none()
    );
    catalog
        .finish_creation(&scope, reserved.id)
        .expect("finish creation");
    assert_eq!(
        other.get(&scope, reserved.id).expect("ready visible"),
        Some(reserved.clone())
    );
    assert!(other.delete(&scope, resource.id).expect("delete"));
    assert!(
        catalog
            .is_tombstoned(&scope, resource.id)
            .expect("shared tombstone")
    );
    assert_eq!(
        catalog
            .owner_resources("owner")
            .expect("owner cleanup inventory")
            .len(),
        2
    );
    let results = database
        .client()
        .execute_sql("SELECT * FROM resources", None)
        .await
        .expect("actual SQL metadata");
    assert_eq!(results[0].rows.len(), 2);
    assert!(!root.path().join("first/databases").exists());
    assert!(!root.path().join("other/databases").exists());
}

#[tokio::test]
async fn sql_runtime_uses_the_catalog_but_keeps_resource_contents_in_files() {
    let root = tempfile::tempdir().expect("resource root");
    let database = MemoryDatabase::new().await.expect("owner schema");
    let scope = Scope("owner", Id::now_v7());
    let runtime =
        DatabaseRuntime::with_client(root.path().to_owned(), database.client()).expect("runtime");
    let resource = runtime
        .ensure(&scope, "key", Some("SQL content".into()))
        .expect("ensure SQL database");
    let content = runtime
        .open_existing(&scope, resource.id)
        .expect("resource file")
        .expect("opened resource");
    content
        .client()
        .execute_sql("CREATE TABLE example (id INTEGER PRIMARY KEY)", None)
        .await
        .expect("resource contents");
    drop(content);
    drop(runtime);
    let reopened =
        DatabaseRuntime::with_client(root.path().to_owned(), database.client()).expect("reopen");
    assert_eq!(
        reopened
            .ensure(&scope, "key", None)
            .expect("idempotent ensure"),
        resource
    );
    assert!(
        reopened
            .open_existing(&scope, resource.id)
            .expect("persistent file")
            .is_some()
    );
}

#[tokio::test]
async fn local_seed_is_repeatable_and_supports_real_admin_credentials_and_public_frontends() {
    use idp_service::{
        PasswordConfig,
        oauth2::{OAuth2Config, OAuth2Service},
        replica::{
            DbApplicationRepo, DbClientRepo, DbKeyRepo, DbOAuth2AuthorizationCodeRepo,
            DbOAuth2RefreshTokenRepo, DbOAuth2UserConsentRepo, DbUserRepo,
        },
        repo::{KeyService, PrivateKeyKeyringRepo, UserRepo},
    };
    let root = tempfile::tempdir().expect("local keys root");
    let database = MemoryDatabase::new().await.expect("schema");
    let client = database.client();
    let issuer = "http://127.0.0.1:1355/idp";
    let namespace = "owner-local-validation";
    let key_dir = root.path().join("keys");
    for _ in 0..2 {
        let binding = crate::local_seed::signer(client.clone(), &key_dir, namespace, issuer)
            .await
            .expect("local signer without setup");
        let keys = Arc::new(KeyService::new(
            DbKeyRepo::new(client.clone()),
            PrivateKeyKeyringRepo::new_local(&key_dir).expect("local keys"),
            namespace,
        ));
        let service = Arc::new(OAuth2Service::new(
            DbApplicationRepo::new(client.clone()),
            DbClientRepo::new(client.clone(), keys.clone()),
            DbOAuth2AuthorizationCodeRepo::new(client.clone()),
            DbOAuth2RefreshTokenRepo::new(client.clone()),
            DbUserRepo::new(client.clone(), PasswordConfig::default()),
            DbOAuth2UserConsentRepo::new(client.clone()),
            keys,
            OAuth2Config {
                issuer: issuer.into(),
                local_signer: Some(binding),
                ..Default::default()
            },
        ));
        let state = crate::RouterState {
            api_base_uri: issuer.into(),
            engine: client.clone(),
            oauth2_service: service,
        };
        crate::local_seed::seed(&state, "http://localhost:5173", 1355)
            .await
            .expect("local seed");
        use of_model::idp::contract::{
            ClientCredentialsGrantRequest, OAuth2ClientAuth, TokenEndpointAuthMethod, TokenRequest,
        };
        let token = state
            .oauth2_service
            .token(
                TokenRequest::ClientCredentials(ClientCredentialsGrantRequest {
                    client_id: "local-storage".into(),
                    client_secret: "local-password".into(),
                    scope: Some("idp.token.validate".into()),
                    audience: Some(issuer.into()),
                    resource: None,
                }),
                Some(OAuth2ClientAuth {
                    client_id: "local-storage".into(),
                    client_secret: Some("local-password".into()),
                    method: TokenEndpointAuthMethod::ClientSecretBasic,
                }),
            )
            .await
            .expect("local service can obtain a signed token");
        assert!(!token.access_token.0.is_empty());
        let admin = state
            .oauth2_service
            .user_repo
            .find_user_by_username_or_email("admin@local.test")
            .await
            .expect("lookup admin")
            .expect("admin");
        let password = state
            .oauth2_service
            .user_repo
            .find_user_password_by_user_id(admin.id)
            .await
            .expect("password")
            .expect("credential");
        assert!(
            argon2::verify_encoded(&password.password_hash, b"password")
                .expect("verify local password")
        );
    }
    let users = client
        .execute_sql("SELECT * FROM users", None)
        .await
        .expect("users");
    assert_eq!(users[0].rows.len(), 1);
    let frontends = client.execute_sql("SELECT client_id, redirect_uris, client_type FROM clients WHERE client_id = 'app' OR client_id = 'password-manager' ORDER BY client_id", None).await.expect("frontends");
    assert_eq!(frontends[0].rows.len(), 2);
    let text = format!("{:?}", frontends[0].rows);
    assert!(text.contains("http://localhost:5173/callback"));
    assert!(text.contains("http://localhost:5174/callback"));
}
