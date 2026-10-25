use std::{
    collections::HashMap, fs, io::Write, os::unix::fs::OpenOptionsExt, path::PathBuf, sync::Arc,
};

use idp_model::model::Id;
use idp_service::{
    oauth2::OAuth2Service,
    replica::{
        DbApplicationRepo, DbClientRepo, DbKeyRepo, DbOAuth2AuthorizationCodeRepo,
        DbOAuth2RefreshTokenRepo, DbOAuth2UserConsentRepo, DbUserRepo,
    },
    repo::{KeyService, PrivateKeyKeyringRepo},
};
use tokio::net::TcpListener;
use unified_server::{RuntimeOwners, UnifiedConfig, UnifiedEndpoint, UnifiedRuntime};

pub struct Fixture {
    pub root: PathBuf,
    pub config: UnifiedConfig,
    pub password: PathBuf,
    pub store: Arc<keyring_core::CredentialStore>,
}

impl Fixture {
    pub fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("unified-local-provisioning-{}", Id::now_v7()));
        api::setup_files::create_private_directory(&root, false).expect("private host parent");
        let password = root.join("initial-password");
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&password)
            .expect("protected password input")
            .write_all(b"fixture-initial-password\n")
            .expect("write test password");
        let mut idp = idp_server::AppConfig::default();
        idp.data_dir = root.join("idp").to_string_lossy().into_owned();
        idp.oauth2.issuer = format!("https://installation-{}.test/idp", Id::now_v7());
        idp.api_public_uri = "https://identity.test".into();
        idp.service_audience = Some("https://identity.test/service".into());
        idp.password.memory_mib = 1;
        idp.password.iterations = 1;
        let management = management_server::AppConfig {
            data_dir: root.join("management").to_string_lossy().into_owned(),
            api_public_uri: "https://management.test".into(),
            storage_audience: "https://storage.test".into(),
            ..Default::default()
        };
        let storage = storage_server::AppConfig {
            data_dir: root.join("storage").to_string_lossy().into_owned(),
            api_public_uri: "https://storage.test".into(),
            ..Default::default()
        };
        let config = UnifiedConfig {
            listen_addr: "127.0.0.1:0".parse().expect("local ephemeral listener"),
            endpoint_data_dir: root.join("endpoint"),
            idp,
            device: device_server::AppConfig {
                data_dir: root.join("device").to_string_lossy().into_owned(),
                storage_audience: management.storage_audience.clone(),
                ..Default::default()
            },
            management,
            storage,
        };
        let store: Arc<keyring_core::CredentialStore> =
            keyring_core::mock::Store::new().expect("isolated Node credential store");
        Self {
            root,
            config,
            password,
            store,
        }
    }

    pub fn journal(&self) -> serde_json::Value {
        serde_json::from_slice(
            &fs::read(
                self.config
                    .local_provisioning_directory()
                    .join("intent.json"),
            )
            .expect("retained host intent"),
        )
        .expect("public intent format")
    }

    pub fn secrets(&self) -> Vec<Vec<u8>> {
        let mut secrets = self
            .store
            .search(&HashMap::new())
            .expect("fixture provider inventory")
            .into_iter()
            .filter_map(|entry| entry.get_secret().ok())
            .collect::<Vec<_>>();
        secrets.sort();
        secrets
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("remove only isolated fixture");
    }
}

pub async fn rotation_login_client(
    fixture: &Fixture,
    grants: Vec<idp_model::contract::GrantType>,
    audiences: Vec<String>,
) -> idp_model::contract::ClientRegistration {
    let owner = idp_server::OwnerProvisioner::open(&fixture.config.idp)
        .await
        .expect("stopped fixture IdP owner");
    let mut request = fixture
        .config
        .idp
        .operator_login_registration(Id::now_v7(), audiences);
    request.allowed_grant_types = grants;
    let engine = owner.node_join_database().expect("local identity owner");
    let keys = Arc::new(KeyService::new(
        DbKeyRepo::new(engine.client()),
        PrivateKeyKeyringRepo::new(&fixture.config.idp.oauth2.issuer)
            .expect("retained fixture private keys"),
        fixture.config.idp.key_namespace.clone(),
    ));
    OAuth2Service::new(
        DbApplicationRepo::new(engine.client()),
        DbClientRepo::new(engine.client(), Arc::clone(&keys)),
        DbOAuth2AuthorizationCodeRepo::new(engine.client()),
        DbOAuth2RefreshTokenRepo::new(engine.client()),
        DbUserRepo::new(engine.client(), fixture.config.idp.password.clone()),
        DbOAuth2UserConsentRepo::new(engine.client()),
        keys,
        fixture.config.idp.oauth2.clone(),
    )
    .register_client(request)
    .await
    .expect("independent public login fixture")
}

pub struct NodeStores {
    pub stores: [(String, Arc<keyring_core::CredentialStore>); 2],
}

impl keyring_core::api::CredentialStoreApi for NodeStores {
    fn vendor(&self) -> String {
        "isolated-full-node-test-stores".into()
    }

    fn id(&self) -> String {
        self.vendor()
    }

    fn build(
        &self,
        service: &str,
        user: &str,
        _: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<keyring_core::Entry> {
        let (_, store) = self
            .stores
            .iter()
            .find(|(namespace, _)| user.starts_with(&format!("{namespace}:")))
            .ok_or(keyring_core::Error::NoEntry)?;
        keyring_core::api::CredentialStoreApi::build(&**store, service, user, None)
    }

    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<keyring_core::Entry>> {
        let mut entries = Vec::new();
        for (_, store) in &self.stores {
            entries.extend(store.search(spec)?);
        }
        Ok(entries)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub async fn runtime(config: &UnifiedConfig) -> (UnifiedRuntime, RuntimeOwners) {
    let owners = RuntimeOwners {
        endpoint: Arc::new(
            UnifiedEndpoint::open(&config.endpoint_data_dir, &config.idp)
                .await
                .expect("retained Node endpoint"),
        ),
        idp: idp_server::open_local_identity_database(std::path::Path::new(&config.idp.data_dir))
            .await
            .expect("retained identity owner"),
        devices: device_server::open_device_database(&config.device)
            .await
            .expect("retained Device owner"),
    };
    let listener = TcpListener::bind(config.listen_addr)
        .await
        .expect("ephemeral host listener");
    let base = format!("http://{}", listener.local_addr().expect("host address"));
    let runtime = UnifiedRuntime::build_with_owners(
        config.clone(),
        Some(listener),
        base,
        RuntimeOwners {
            endpoint: Arc::clone(&owners.endpoint),
            idp: Arc::clone(&owners.idp),
            devices: Arc::clone(&owners.devices),
        },
    )
    .await
    .expect("host with fixture-owned public owners");
    (runtime, owners)
}

pub fn run(scenario: impl std::future::Future<Output = ()> + Send + 'static) {
    static CREDENTIAL_STORE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _credential_store = CREDENTIAL_STORE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // The credential provider is process-global; native imports also need a larger stack.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_stack_size(64 * 1024 * 1024)
                .enable_all()
                .build()
                .expect("full-node test runtime")
                .block_on(async {
                    tokio::time::timeout(std::time::Duration::from_secs(600), scenario)
                        .await
                        .expect("bounded full-node scenario");
                });
        })
        .expect("full-node test thread")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}

#[test]
fn fixture_retains_owner_only_root_and_password() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    assert_eq!(
        fs::metadata(&fixture.root)
            .expect("fixture root")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&fixture.password)
            .expect("password input")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn node_stores_ignore_modifiers_without_crossing_namespaces() {
    use keyring_core::api::CredentialStoreApi;

    let left = keyring_core::mock::Store::new().expect("left private provider");
    let right = keyring_core::mock::Store::new().expect("right private provider");
    let stores = NodeStores {
        stores: [
            ("left".into(), left.clone()),
            ("right".into(), right.clone()),
        ],
    };
    let modifiers = HashMap::from([("test-only-modifier", "ignored-by-mock")]);
    for (namespace, secret) in [
        ("left", b"left-secret".as_slice()),
        ("right", b"right-secret".as_slice()),
    ] {
        let entry = stores
            .build(
                "fixture",
                &format!("{namespace}:node:signer"),
                Some(&modifiers),
            )
            .expect("mock ignores provider modifiers");
        entry.set_secret(secret).expect("retain namespace secret");
        assert_eq!(entry.get_secret().expect("retained secret"), secret);
    }
    assert_eq!(
        left.search(&HashMap::new()).expect("left inventory").len(),
        1
    );
    assert_eq!(
        right
            .search(&HashMap::new())
            .expect("right inventory")
            .len(),
        1
    );
    assert!(matches!(
        stores.build("fixture", "left-other:node:signer", Some(&modifiers)),
        Err(keyring_core::Error::NoEntry)
    ));
}
