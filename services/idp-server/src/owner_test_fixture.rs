use std::sync::Arc;

use idp_service::oauth2::node_signer::NodeSignerBinding;
use idp_service::repo::client_repo::ClientRepo;
use iroh::SecretKey;
use of_model::id::Id;
use of_model::idp::contract::application_registration::ApplicationRegistration;
use of_model::idp::contract::client_profile::ClientProfile;
use of_model::idp::contract::client_registration::ClientRegistration;
use of_model::idp::contract::client_type::ClientType;
use of_model::idp::contract::grant_type::GrantType;
use of_model::idp::contract::oauth2_client_auth::OAuth2ClientAuth;
use of_model::idp::contract::password_grant_request::PasswordGrantRequest;
use of_model::idp::contract::token_endpoint_auth_method::TokenEndpointAuthMethod;
use of_model::idp::contract::token_request::TokenRequest;

use crate::{config::app_config::AppConfig, provisioning::OwnerProvisioner};

pub(crate) struct OwnerFixture {
    pub(crate) state: crate::router::state::RouterState,
    pub(crate) server: iroh_chain::Server,
    pub(crate) issuer: String,
    pub(crate) root: std::path::PathBuf,
    pub(crate) owner: Id,
    pub(crate) management: Arc<ofdb_sql::Database>,
}

pub(crate) async fn owner_fixture() -> OwnerFixture {
    owner_fixture_for_issuer(format!("https://installation-{}.test/idp", Id::now_v7())).await
}

pub(crate) async fn owner_fixture_for_issuer(issuer: String) -> OwnerFixture {
    let root = std::env::temp_dir().join(format!("idp-owner-test-{}", Id::now_v7()));
    let mut config = AppConfig {
        data_dir: root.to_string_lossy().into_owned(),
        ..AppConfig::default()
    };
    config.oauth2.issuer = issuer.clone();
    let store = TestStore::new();
    let mut provisioner = OwnerProvisioner::open_with_private_keys(
        &config,
        idp_service::repo::private_key_keyring_repo::PrivateKeyKeyringRepo::new_with_store(
            &issuer,
            Arc::clone(&store),
        ),
    )
    .await
    .expect("native owner");
    let secret = SecretKey::generate();
    let server = iroh_chain::Server::bind_with_secret_key(
        iroh::endpoint::presets::Minimal,
        secret.clone(),
        iroh_chain::EndpointIdStore::default(),
    )
    .await
    .expect("peer endpoint");
    let owner = Id::now_v7();
    provisioner
        .prepare_fresh_setup_secret_inventory(owner, &[])
        .await
        .expect("fresh approval owner secret inventory before mutation");
    provisioner
        .prepare_setup_client_slots(&[])
        .await
        .expect("ready approval owner secret inventory");
    provisioner
        .ensure_initial_user(owner, "admin", "valid-password")
        .await
        .expect("initial owner");
    provisioner
        .oauth2
        .key_service
        .ensure_entity_master_key(
            of_model::idp::contract::entity_type::EntityType::User,
            owner,
            "valid-password",
        )
        .expect("User local master key");
    provisioner
        .oauth2
        .key_service
        .create_key(
            None,
            of_model::idp::contract::entity_type::EntityType::User,
            owner,
            true,
            "User session key".into(),
            None,
        )
        .await
        .expect("real User session signing key");
    let peer_binding = NodeSignerBinding {
        member_id: Id::now_v7(),
        key_id: Id::now_v7(),
        endpoint_id: server.endpoint().id().to_string(),
    };
    provisioner.oauth2.oauth_config.local_signer = Some(peer_binding.clone());
    provisioner
        .ensure_initial_node(&peer_binding, owner, "Approving Node")
        .await
        .expect("independent trusted peer signer");
    provisioner
        .oauth2
        .client_repo
        .create_client(ClientRegistration {
            application: ApplicationRegistration {
                name: Some("Test User session".into()),
                uri: "https://approval.test".into(),
                description: None,
            },
            client_id: Some("human-client".into()),
            client_secret: Some("human-secret".into()),
            client_id_issued_at: None,
            client_secret_expires_at: None,
            client_name: "Test User session".into(),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            terms_of_service_uri: None,
            policy_uri: None,
            client_type: ClientType::Confidential,
            profile: ClientProfile::Web,
            redirect_uris: vec![],
            allowed_grant_types: vec![GrantType::Password, GrantType::TokenExchange],
            response_types: vec![],
            allowed_scopes: vec!["openid".into()],
            allowed_audiences: vec![
                "idp-services".into(),
                "wrong-audience".into(),
                management_service::service::MANAGEMENT_APPLICATION_URI.into(),
            ],
            token_endpoint_auth_method: TokenEndpointAuthMethod::ClientSecretBasic,
            software_statement: None,
            software_id: None,
            software_version: None,
        })
        .await
        .expect("human session client");
    std::fs::create_dir_all(root.join("management")).expect("separate policy directory");
    let management = Arc::new(
        ofdb_sql::Database::open(root.join("management/management.redb"))
            .expect("native Management owner"),
    );
    db::ofdb::up(&management.client())
        .await
        .expect("identity policy schema");
    device_service::ofdb::schema::up(&management)
        .await
        .expect("device schema");
    let devices = device_service::ofdb::DbDeviceRepo::new(management.client());
    use device_service::DeviceRepo;
    let device = devices
        .create(
            owner.to_string(),
            "First Device".into(),
            peer_binding.endpoint_id,
            String::new(),
            Vec::new(),
            0,
        )
        .await
        .expect("first fixture Device");
    assert_eq!(device.state, device_service::DeviceState::Approved);
    let engine = provisioner.engine.clone();
    let state = crate::router::state::RouterState::new(
        "https://ui.test",
        "idp-services",
        engine.clone(),
        Arc::new(provisioner.oauth2),
        Some(Arc::new(device_service::ofdb::DbDeviceRepo::new(
            management.clone().client(),
        ))),
        Arc::new(
            device_service::device_identity_from_server(&server, secret).expect("peer identity"),
        ),
    )
    .with_local_permissions(management_service::LocalPermissions::new(
        management.client(),
    ));
    OwnerFixture {
        state,
        server,
        issuer,
        root,
        owner,
        management,
    }
}

pub(crate) async fn user_token(fixture: &OwnerFixture, audience: &str) -> String {
    let token = fixture
        .state
        .oauth2_service
        .token(
            TokenRequest::Password(PasswordGrantRequest {
                client_id: "human-client".into(),
                username: "admin".into(),
                password: "valid-password".into(),
                scope: Some("openid".into()),
                resource: Some(audience.into()),
            }),
            Some(OAuth2ClientAuth {
                client_id: "human-client".into(),
                client_secret: Some("human-secret".into()),
                method: TokenEndpointAuthMethod::ClientSecretBasic,
            }),
        )
        .await
        .expect("production User session token")
        .access_token
        .0;
    fixture
        .state
        .oauth2_service
        .token(
            TokenRequest::TokenExchange(
                of_model::idp::contract::token_exchange_grant_request::TokenExchangeGrantRequest {
                    subject_token: token,
                    subject_token_type:
                        of_model::idp::contract::subject_token_type::SubjectTokenType::AccessToken,
                    actor_token: None,
                    actor_token_type: None,
                    resource: Some(audience.into()),
                    audience: None,
                    authorization_details: None,
                    scope: None,
                    requested_token_type: None,
                },
            ),
            Some(OAuth2ClientAuth {
                client_id: "human-client".into(),
                client_secret: Some("human-secret".into()),
                method: TokenEndpointAuthMethod::ClientSecretBasic,
            }),
        )
        .await
        .expect("purpose-audience User session token")
        .access_token
        .0
}

pub(crate) struct TestStore(std::sync::Arc<keyring_core::mock::Store>);
impl TestStore {
    pub(crate) fn new() -> Arc<keyring_core::CredentialStore> {
        Arc::new(Self(
            keyring_core::mock::Store::new().expect("test keyring"),
        ))
    }
}
impl keyring_core::api::CredentialStoreApi for TestStore {
    fn vendor(&self) -> String {
        self.0.vendor()
    }
    fn id(&self) -> String {
        self.0.id()
    }
    fn build(
        &self,
        service: &str,
        user: &str,
        _: Option<&std::collections::HashMap<&str, &str>>,
    ) -> keyring_core::Result<keyring_core::Entry> {
        keyring_core::api::CredentialStoreApi::build(&*self.0, service, user, None)
    }
    fn search(
        &self,
        spec: &std::collections::HashMap<&str, &str>,
    ) -> keyring_core::Result<Vec<keyring_core::Entry>> {
        self.0.search(spec)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
