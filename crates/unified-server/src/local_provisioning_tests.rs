use std::{
    collections::HashMap,
    fs,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
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

use crate::{LocalProvisioner, UnifiedConfig};

#[cfg(target_os = "linux")]
#[path = "https_bootstrap_tests.rs"]
mod https_bootstrap_tests;

pub(crate) struct SecretStore {
    pub(crate) store: Arc<keyring_core::mock::Store>,
    pub(crate) fail_node_readback: Arc<AtomicBool>,
}
impl keyring_core::api::CredentialStoreApi for SecretStore {
    fn vendor(&self) -> String {
        self.store.vendor()
    }
    fn id(&self) -> String {
        self.store.id()
    }
    fn build(
        &self,
        service: &str,
        user: &str,
        _: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<keyring_core::Entry> {
        let entry =
            keyring_core::api::CredentialStoreApi::build(&*self.store, service, user, None)?;
        if user.contains(":node:")
            && entry.get_secret().is_ok()
            && self.fail_node_readback.swap(false, Ordering::SeqCst)
        {
            return Err(keyring_core::Error::NoEntry);
        }
        Ok(entry)
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<keyring_core::Entry>> {
        self.store.search(spec)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct Fixture {
    root: PathBuf,
    config: UnifiedConfig,
    password: PathBuf,
    store: Arc<keyring_core::CredentialStore>,
    fail_node_readback: Arc<AtomicBool>,
}

impl Fixture {
    fn new() -> Self {
        use std::io::Write;
        let root =
            std::env::temp_dir().join(format!("unified-local-provisioning-{}", Id::now_v7()));
        fs::create_dir(&root).expect("isolated host parent");
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
        let fail_node_readback = Arc::new(AtomicBool::new(false));
        let store: Arc<keyring_core::CredentialStore> = Arc::new(SecretStore {
            store: keyring_core::mock::Store::new().expect("credential adapter"),
            fail_node_readback: fail_node_readback.clone(),
        });
        keyring_core::set_default_store(store.clone());
        Self {
            root,
            config,
            password,
            store,
            fail_node_readback,
        }
    }

    fn host_store(&self) -> crate::host_store::HostStore {
        crate::host_store::HostStore::for_config(&self.config).expect("retained host SQL")
    }

    fn journal(&self) -> serde_json::Value {
        self.host_store()
            .get("create")
            .expect("read Create")
            .expect("retained Create")
    }

    fn write_journal(&self, value: &serde_json::Value) {
        self.host_store()
            .put("create", value)
            .expect("inject Create SQL fault");
    }

    fn endpoint_entry(&self) -> keyring_core::Entry {
        let reference = &self.journal()["endpoint_secret"];
        self.store
            .build(
                reference["service"].as_str().expect("service"),
                reference["entry"].as_str().expect("exact entry"),
                None,
            )
            .expect("injected endpoint entry")
    }

    fn secrets(&self) -> Vec<Vec<u8>> {
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

#[tokio::test]
async fn endpoint_private_material_is_provider_owned_and_stable_on_reopen() {
    let fixture = Fixture::new();
    let mut local = LocalProvisioner::create_with_password(
        fixture.config.clone(),
        "operator",
        "Node",
        "fixture-initial-password",
    )
    .expect("fresh host intent");
    let runtime = local
        .prepare_hosted_runtime(
            "fixture-initial-password",
            false,
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("prepare host");
    let endpoint = runtime.endpoint_id();
    local
        .mark_complete()
        .expect("record completed owner preparation");
    runtime.shutdown().await.expect("stop prepared host");
    assert!(
        !fixture
            .config
            .endpoint_data_dir
            .join("endpoint.key")
            .exists()
    );
    assert!(fixture.secrets().iter().any(|secret| secret.len() == 32));
    let reopened = local
        .reopen_hosted_runtime(false)
        .await
        .expect("reopen exact endpoint");
    assert_eq!(reopened.endpoint_id(), endpoint);
    reopened.shutdown().await.expect("stop reopened host");
}

#[test]
fn identity_settings_errors_preserve_host_validation_order_without_writing_intent() {
    let fixture = Fixture::new();
    let mut config = fixture.config.clone();
    config.idp.oauth2.issuer.clear();
    config.idp.key_namespace.clear();
    config.idp.service_audience = Some(" ".into());
    config.device.data_dir = config.idp.data_dir.clone();
    let rejected = |config| {
        LocalProvisioner::create_with_password(
            config,
            "operator",
            "Node",
            "fixture-initial-password",
        )
        .err()
        .expect("invalid host settings deny")
    };
    let error = rejected(config.clone());
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(error.to_string(), "IdP service audience must not be empty");
    config.idp.service_audience = fixture.config.idp.service_audience.clone();
    let error = rejected(config.clone());
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(
        error.to_string(),
        "context, endpoint and provisioning roots must not overlap"
    );
    config.device.data_dir = fixture.config.device.data_dir.clone();
    let error = rejected(config);
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(
        error.to_string(),
        "Canonical Issuer and key namespace are required"
    );
    for path in [
        &fixture.config.endpoint_data_dir,
        &fixture.config.local_provisioning_directory(),
        Path::new(&fixture.config.idp.data_dir),
    ] {
        assert!(
            !path.exists(),
            "rejected settings cannot create owner or host state"
        );
    }
}

#[test]
fn in_memory_password_create_retains_only_its_verifier() {
    let fixture = Fixture::new();
    let password = "hidden-terminal-initial-password";
    let local = LocalProvisioner::create_with_password(
        fixture.config.clone(),
        "operator",
        "Local Node",
        password,
    )
    .expect("in-memory secure prompt seam");
    assert!(!local.is_complete().expect("explicit incomplete intent"));
    let login = local
        .login_client()
        .expect("public bootstrap login registration");
    assert!(login.client_secret.is_none());
    assert_eq!(login.client_type, idp_model::contract::ClientType::Public);
    assert_eq!(login.profile, idp_model::contract::ClientProfile::Native);
    assert_eq!(
        login.allowed_grant_types,
        [
            idp_model::contract::GrantType::Password,
            idp_model::contract::GrantType::RefreshToken,
            idp_model::contract::GrantType::TokenExchange,
        ]
    );
    assert_eq!(login.allowed_scopes, ["openid", "profile", "email"]);
    assert_eq!(
        login.allowed_audiences,
        [
            "https://identity.test/service",
            "idp-management",
            "https://storage.test",
            "storage:databases",
            "storage:filesystems",
        ]
    );
    assert!(fixture.journal()["intent"].get("clients").is_none());
    drop(local);
    let reopened =
        LocalProvisioner::open(fixture.config.clone()).expect("retain exact create intent");
    assert_eq!(
        reopened
            .login_client()
            .expect("stable public login ID on retry"),
        login
    );
    assert!(!fixture.journal().to_string().contains(password));
    secret_files_absent(&fixture.root);
}

#[test]
fn live_credential_guard_refuses_missing_or_changed_intent_before_provider_work() {
    let fixture = Fixture::new();
    let config = crate::config::test_config(&fixture.root);
    let credentials = crate::CredentialBootstrap::create(config.clone()).expect("fresh references");
    let store = crate::host_store::HostStore::for_config(&config).expect("host SQL");
    let retained: serde_json::Value = store.get("credentials").expect("read").expect("references");
    let secrets = fixture.secrets();
    store
        .update(|records| {
            records.remove("credentials");
            Ok(())
        })
        .expect("remove references");
    assert!(
        credentials.load().is_err(),
        "absence cannot authorize replacement"
    );
    assert!(
        store
            .get::<serde_json::Value>("credentials")
            .expect("read")
            .is_none()
    );
    store
        .put("credentials", &retained)
        .expect("restore checkpoint");
    store
        .put("credentials", &serde_json::json!({"invalid": true}))
        .expect("corrupt references");
    assert!(
        credentials.load().is_err(),
        "changed references cannot be overwritten"
    );
    assert_eq!(
        store.get::<serde_json::Value>("credentials").expect("read"),
        Some(serde_json::json!({"invalid": true}))
    );
    assert!(
        fixture.secrets() == secrets,
        "denial performs no provider production"
    );
}

#[test]
fn old_one_two_and_four_relationship_references_deny_without_rewriting_or_provider_work() {
    for old_count in [1, 2, 4] {
        let fixture = Fixture::new();
        let automatic = fixture.config.clone();
        drop(
            crate::CredentialBootstrap::create(automatic.clone()).expect("fresh public references"),
        );
        let store = crate::host_store::HostStore::for_config(&automatic).expect("host SQL");
        let mut old: serde_json::Value =
            store.get("credentials").expect("read").expect("references");
        assert!(old.get("clients").is_none());
        assert_eq!(old["roots"].as_array().expect("roots").len(), 4);
        old["namespace"] = serde_json::json!(Id::now_v7());
        old["clients"] =
            serde_json::json!((0..old_count).map(|_| Id::now_v7()).collect::<Vec<_>>());
        old["attempted"] = serde_json::json!(vec![true; old_count]);
        old["audiences"] = serde_json::json!(vec![automatic.idp_audience(); old_count]);
        store
            .put("credentials", &old)
            .expect("retain old relationship evidence");
        let secrets = fixture.secrets();
        assert!(crate::CredentialBootstrap::open(automatic).is_err());
        assert_eq!(
            store.get::<serde_json::Value>("credentials").expect("read"),
            Some(old)
        );
        assert!(fixture.secrets() == secrets, "provider inventory changed");
    }
}

#[test]
fn old_one_two_and_four_relationship_create_and_join_intents_deny_unchanged() {
    for old_count in [1, 2, 4] {
        for joining in [false, true] {
            let fixture = Fixture::new();
            let (name, field) = if joining {
                let peer = iroh::SecretKey::generate().public().to_string();
                drop(
                    crate::LocalJoin::create(fixture.config.clone(), &peer, "Joining Node")
                        .expect("fresh Join"),
                );
                ("join.json", "configuration")
            } else {
                drop(
                    LocalProvisioner::create(
                        fixture.config.clone(),
                        "operator",
                        "Node",
                        &fixture.password,
                    )
                    .expect("fresh Create"),
                );
                ("intent.json", "intent")
            };
            let _ = name;
            let mut old: serde_json::Value = if joining {
                crate::host_store::HostStore::for_config(&fixture.config)
                    .expect("host SQL")
                    .get("join")
                    .expect("SQL read")
                    .expect("Join")
            } else {
                fixture.journal()
            };
            assert!(old[field].get("clients").is_none());
            assert_eq!(old[field]["roots"].as_array().expect("roots").len(), 4);
            old[field]["clients"] = serde_json::json!(
                (0..old_count)
                    .map(|_| {
                        crate::local_provisioning::login_registration(&fixture.config, Id::now_v7())
                    })
                    .collect::<Vec<_>>()
            );
            old["secret_verifiers"] =
                serde_json::json!(vec!["retained-obsolete-verifier"; old_count]);
            if joining {
                crate::host_store::HostStore::for_config(&fixture.config)
                    .expect("host SQL")
                    .put("join", &old)
                    .expect("old Join intent");
            } else {
                fixture.write_journal(&old);
            }
            let secrets = fixture.secrets();
            if joining {
                assert!(crate::LocalJoin::open(fixture.config.clone()).is_err());
            } else {
                assert!(LocalProvisioner::open(fixture.config.clone()).is_err());
            }
            if joining {
                assert_eq!(
                    crate::host_store::HostStore::for_config(&fixture.config)
                        .expect("host SQL")
                        .get::<serde_json::Value>("join")
                        .expect("SQL read")
                        .expect("Join"),
                    old
                );
            } else {
                assert_eq!(fixture.journal(), old);
            }
            assert!(fixture.secrets() == secrets, "provider inventory changed");
        }
    }
}

#[test]
fn host_bootstrap_reopens_without_relationship_secrets_or_provider_work() {
    let fixture = Fixture::new();
    let secrets = fixture.secrets();
    let bootstrap =
        crate::CredentialBootstrap::create(fixture.config.clone()).expect("fresh host references");
    let prepared = bootstrap.load().expect("host settings");
    drop(bootstrap);
    let retained = crate::CredentialBootstrap::open(fixture.config.clone())
        .expect("retained host references")
        .load()
        .expect("offline settings");
    assert_eq!(retained.idp.oauth2.issuer, prepared.idp.oauth2.issuer);
    assert!(fixture.secrets() == secrets, "provider inventory changed");
    secret_files_absent(&fixture.root);
}

fn secret_files_absent(root: &Path) {
    assert!(
        !root.join("endpoint/endpoint.key").exists(),
        "no private endpoint production before explicit provision"
    );
    assert!(
        !root.join("idp/idp.redb").exists(),
        "no owner writes before explicit provision"
    );
}

#[tokio::test]
async fn bundled_management_direct_idp_rechecks_registration_and_device_owner_without_http() {
    use idp_model::contract::{
        PasswordGrantRequest, SubjectTokenType, TokenExchangeGrantRequest, TokenRequest,
    };
    use idp_service::repo::{ClientRepo, UserRepo};

    let fixture = Fixture::new();
    let mut local = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Direct Management",
        &fixture.password,
    )
    .expect("fresh local intent");
    local
        .provision(&fixture.password)
        .await
        .expect("real owner seed");
    let login = local
        .login_client()
        .expect("public login")
        .client_id
        .expect("login ID");
    let runtime = local.build_runtime().await.expect("unserved local runtime");
    let owner = runtime.join_owner();
    let journal = fixture.journal();
    let mut config = fixture.config.idp.clone();
    config.oauth2.local_signer = Some(idp_service::oauth2::NodeSignerBinding {
        member_id: serde_json::from_value(journal["member"].clone()).expect("retained member ID"),
        key_id: serde_json::from_value(journal["key"].clone()).expect("retained signer ID"),
        endpoint_id: runtime.endpoint_id().to_string(),
    });
    let oauth2 = fixture_oauth2(
        &config,
        owner.node_join_database().expect("local identity owner"),
    );
    let session = oauth2
        .token(
            TokenRequest::Password(PasswordGrantRequest {
                client_id: login.clone(),
                username: "operator".into(),
                password: "fixture-initial-password".into(),
                scope: Some("openid".into()),
                resource: None,
            }),
            None,
        )
        .await
        .expect("real User login");
    let token = oauth2
        .token(
            TokenRequest::TokenExchange(TokenExchangeGrantRequest {
                subject_token: session.access_token.0,
                subject_token_type: SubjectTokenType::AccessToken,
                actor_token: None,
                actor_token_type: None,
                resource: Some(management_service::MANAGEMENT_APPLICATION_URI.into()),
                audience: None,
                authorization_details: None,
                scope: None,
                requested_token_type: None,
            }),
            None,
        )
        .await
        .expect("Management User bearer")
        .access_token
        .0;
    let control = &runtime.management_control_plane;
    let (claims, _) = control
        .validate_actor_token(&token)
        .await
        .expect("direct current bearer validation");
    let user: Id = claims.sub.parse().expect("User ID");
    let endpoint = runtime.endpoint_id().to_string();
    let identity = control
        .get_storage_endpoint_identity(&endpoint)
        .await
        .expect("direct approved endpoint");
    assert_eq!(identity.owner_subject, user.to_string());
    assert_eq!(
        control
            .get_storage_device_identity(identity.device_id)
            .await
            .expect("direct Device identity")
            .endpoint_id,
        endpoint
    );
    oauth2
        .client_repo
        .delete_client_by_client_id(&login)
        .await
        .expect("learn login registration deletion");
    assert!(control.validate_actor_token(&token).await.is_err());
    assert!(
        control
            .get_storage_device_identity(identity.device_id)
            .await
            .is_ok(),
        "Device trust is not login Client state"
    );
    oauth2
        .user_repo
        .delete_user_by_id(user)
        .await
        .expect("learn Device owner deletion");
    assert!(
        control
            .get_storage_endpoint_identity(&endpoint)
            .await
            .is_err()
    );
    assert!(
        control
            .get_storage_device_identity(identity.device_id)
            .await
            .is_err()
    );
    drop(oauth2);
    runtime.shutdown().await.expect("close unserved owners");
}

fn fixture_oauth2(
    config: &idp_server::AppConfig,
    engine: Arc<db::Database>,
) -> OAuth2Service<
    DbApplicationRepo,
    DbClientRepo,
    DbOAuth2AuthorizationCodeRepo,
    DbOAuth2RefreshTokenRepo,
    DbUserRepo,
    DbOAuth2UserConsentRepo,
    DbKeyRepo,
    PrivateKeyKeyringRepo,
> {
    let keys = Arc::new(KeyService::new(
        DbKeyRepo::new(engine.client()),
        PrivateKeyKeyringRepo::new(&config.oauth2.issuer).expect("retained fixture private keys"),
        config.key_namespace.clone(),
    ));
    OAuth2Service::new(
        DbApplicationRepo::new(engine.client()),
        DbClientRepo::new(engine.client(), Arc::clone(&keys)),
        DbOAuth2AuthorizationCodeRepo::new(engine.client()),
        DbOAuth2RefreshTokenRepo::new(engine.client()),
        DbUserRepo::new(engine.client(), config.password.clone()),
        DbOAuth2UserConsentRepo::new(engine.client()),
        keys,
        config.oauth2.clone(),
    )
}

async fn exercise_running_host(runtime: crate::UnifiedRuntime, user: Id) {
    let permissions = management_service::LocalPermissions::new(
        runtime
            .join_owner()
            .node_join_database()
            .expect("local identity owner")
            .client(),
    );
    use idp_model::contract::{
        IdentityAction, IdentityResource, PermissionEvaluationRequest, PermissionSubject,
        PermissionTarget,
    };
    let base = format!("http://{}", runtime.local_addr());
    let cancellation = runtime.cancellation_token();
    let task = tokio::spawn(runtime.serve());
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("local authenticated owner API client");
    let mut healthy = false;
    for _ in 0..100 {
        if http
            .get(format!("{base}/idp/health"))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            healthy = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(healthy, "actual host listener starts without peer contact");
    for (action, resource) in [
        (
            IdentityAction::InfrastructureClientsCreate,
            IdentityResource::Client { client_id: None },
        ),
        (
            IdentityAction::DevicePairingUpdate,
            IdentityResource::DevicePairing,
        ),
        (
            IdentityAction::ReplicaSignersEnroll,
            IdentityResource::ReplicaSigner { member_id: None },
        ),
        (
            IdentityAction::IdpSecuritySync,
            IdentityResource::IdpSecurity,
        ),
        (
            IdentityAction::ManagementSecuritySync,
            IdentityResource::ManagementSecurity,
        ),
    ] {
        for (subject, expected) in [(user, true), (Id::now_v7(), false)] {
            let request = PermissionEvaluationRequest {
                request_id: Id::now_v7(),
                subject: PermissionSubject::User { id: subject },
                action,
                target: PermissionTarget::Installation {
                    resource: resource.clone(),
                },
            };
            assert_eq!(
                permissions
                    .evaluate(&request)
                    .await
                    .expect("current owner-local permission decision"),
                expected,
                "grants belong only to the retained initial User"
            );
        }
    }
    let request = PermissionEvaluationRequest {
        request_id: Id::now_v7(),
        subject: PermissionSubject::User { id: user },
        action: IdentityAction::ClientsCreate,
        target: PermissionTarget::Application {
            application_id: Id::now_v7(),
            resource: IdentityResource::Client { client_id: None },
        },
    };
    assert!(
        !permissions
            .evaluate(&request)
            .await
            .expect("application-scoped decision"),
        "initial Installation grants are not a global application role"
    );
    cancellation.cancel();
    tokio::time::timeout(Duration::from_secs(60), task)
        .await
        .expect("bounded actual host shutdown")
        .expect("host task")
        .expect("host serve/shutdown");
}

fn copy_stopped_recovery_tree(source: &Path, destination: &Path) {
    let metadata = fs::symlink_metadata(source).expect("stopped fixture tree metadata");
    assert!(
        !metadata.file_type().is_symlink(),
        "fixture snapshot excludes symlinks"
    );
    if metadata.is_dir() {
        fs::create_dir_all(destination).expect("protected fixture recovery directory");
        fs::set_permissions(destination, metadata.permissions()).expect("retain directory modes");
        for entry in fs::read_dir(source).expect("complete stopped fixture tree") {
            let entry = entry.expect("fixture recovery entry");
            copy_stopped_recovery_tree(&entry.path(), &destination.join(entry.file_name()));
        }
    } else {
        assert!(
            metadata.is_file(),
            "fixture snapshot contains only regular files"
        );
        if source
            .file_name()
            .is_some_and(|name| name == "operation.lock")
            && destination.exists()
        {
            assert!(
                fs::read(source).expect("snapshot lock")
                    == fs::read(destination).expect("retained lock"),
                "retain the original cooperating lock rather than replacing it"
            );
        } else {
            fs::copy(source, destination).expect("copy stopped fixture file without logging bytes");
            fs::set_permissions(destination, metadata.permissions()).expect("retain file modes");
        }
    }
}

fn assert_matching_bytes(expected: &[u8], actual: &[u8], context: &str) {
    let first_difference = expected
        .iter()
        .zip(actual)
        .position(|(expected, actual)| expected != actual)
        .or_else(|| (expected.len() != actual.len()).then_some(expected.len().min(actual.len())));
    assert!(
        first_difference.is_none(),
        "{context}: expected {} bytes, got {} bytes; first difference at offset {first_difference:?}",
        expected.len(),
        actual.len(),
    );
}

#[test]
fn byte_mismatch_diagnostics_do_not_dump_contents() {
    let expected = vec![0xab; 1024 * 1024];
    let mut actual = expected.clone();
    actual[123] = 0xcd;
    let panic = std::panic::catch_unwind(|| {
        assert_matching_bytes(&expected, &actual, "recovery contents");
    })
    .expect_err("different contents must fail");
    let message = panic.downcast_ref::<String>().expect("formatted assertion");
    assert!(message.len() < 200, "byte diagnostics must stay bounded");
    assert!(message.contains("1048576 bytes"));
    assert!(message.contains("offset Some(123)"));
    assert_matching_bytes(&expected, &expected, "identical contents");
}

fn assert_stopped_recovery_tree(source: &Path, restored: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let source_metadata = fs::symlink_metadata(source).expect("snapshot metadata");
    let restored_metadata = fs::symlink_metadata(restored).expect("restored metadata");
    assert_eq!(
        source_metadata.permissions().mode(),
        restored_metadata.permissions().mode()
    );
    assert_eq!(source_metadata.is_dir(), restored_metadata.is_dir());
    assert!(!restored_metadata.file_type().is_symlink());
    if source_metadata.is_dir() {
        let names = |root: &Path| {
            let mut names = fs::read_dir(root)
                .expect("fixture tree entries")
                .map(|entry| entry.expect("fixture entry").file_name())
                .collect::<Vec<_>>();
            names.sort();
            names
        };
        assert_eq!(names(source), names(restored), "complete tree restored");
        for name in names(source) {
            assert_stopped_recovery_tree(&source.join(&name), &restored.join(&name));
        }
    } else if source.file_name().is_some_and(|name| name == "host.redb") {
        let records = |path: &Path| {
            crate::host_store::HostStore::open(path.parent().expect("SQL host directory"))
                .expect("retained SQL")
                .update(|records| Ok(records.clone()))
                .expect("complete logical SQL snapshot")
        };
        assert!(
            records(source) == records(restored),
            "complete logical host SQL restored"
        );
    } else {
        assert_matching_bytes(
            &fs::read(source).expect("snapshot bytes"),
            &fs::read(restored).expect("restored bytes"),
            "restored file contents",
        );
    }
}

#[tokio::test]
async fn local_provisioning_restores_stopped_same_node_filesystem_with_retained_provider() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let fixture = Fixture::new();
    fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o700))
        .expect("protect the entire disposable recovery fixture");
    let mut host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Local Node",
        &fixture.password,
    )
    .expect("explicit isolated Node provisioning");
    host.provision(&fixture.password)
        .await
        .expect("completed current-format Node");
    let journal = fixture.journal();
    let endpoint = crate::UnifiedEndpoint::persisted_endpoint_id(&fixture.config.endpoint_data_dir)
        .expect("original persisted endpoint");
    let binding = idp_service::oauth2::NodeSignerBinding {
        member_id: serde_json::from_value(journal["member"].clone()).expect("retained member"),
        key_id: serde_json::from_value(journal["key"].clone()).expect("retained signer key"),
        endpoint_id: endpoint.to_string(),
    };
    let mut config = fixture.config.clone();
    config.idp.oauth2.local_signer = Some(binding.clone());
    let owner = idp_server::OwnerProvisioner::open(&config.idp)
        .await
        .expect("original owner");
    let signer = owner
        .verify_node_signer(&binding)
        .await
        .expect("original trusted local signer");
    drop(owner);
    let runtime = host
        .build_runtime()
        .await
        .expect("original completed runtime");
    assert!(
        runtime
            .node_public_ready()
            .await
            .expect("original readiness")
    );
    assert_eq!(runtime.endpoint_id(), endpoint);
    runtime
        .shutdown()
        .await
        .expect("stop runtime before snapshot");
    drop(host);
    let secrets = fixture.secrets();
    let roots = [
        PathBuf::from(&config.idp.data_dir),
        PathBuf::from(&config.device.data_dir),
        PathBuf::from(&config.storage.data_dir),
        config.endpoint_data_dir.clone(),
        config.local_provisioning_directory(),
        crate::BootstrapConfig::explicit_directory(&config),
    ];
    let store = crate::host_store::HostStore::for_config(&config).expect("host SQL");
    let logical = store
        .update(|records| Ok(records.clone()))
        .expect("stopped logical operation");
    let snapshot = fixture.root.join("stopped-snapshot");
    let displaced = fixture.root.join("displaced-state");
    for directory in [&snapshot, &displaced] {
        fs::create_dir(directory).expect("isolated recovery parent");
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .expect("protected recovery parent");
    }
    for (index, root) in roots.iter().enumerate() {
        copy_stopped_recovery_tree(root, &snapshot.join(index.to_string()));
        assert_stopped_recovery_tree(root, &snapshot.join(index.to_string()));
        fs::rename(root, displaced.join(index.to_string())).expect("displace stopped fixture root");
    }
    assert!(
        !crate::BootstrapConfig::explicit_directory(&config).exists(),
        "recovery fixture must displace authoritative host SQL, not leave the original journal live"
    );
    assert!(
        LocalProvisioner::open_runtime(config.clone())
            .await
            .is_err(),
        "missing recovery set denies"
    );
    assert!(
        roots.iter().all(|root| !root.exists()),
        "missing roots are not repaired"
    );
    let journal_root = &roots[4];
    fs::create_dir(journal_root).expect("restore journal directory around retained lock");
    fs::set_permissions(journal_root, fs::Permissions::from_mode(0o700))
        .expect("protected journal");
    let lock = journal_root.join("operation.lock");
    fs::rename(displaced.join("4/operation.lock"), &lock).expect("retain original lock inode");
    let lock_identity = fs::metadata(&lock).expect("original lock metadata");
    for (index, root) in roots.iter().enumerate() {
        copy_stopped_recovery_tree(&snapshot.join(index.to_string()), root);
        assert_stopped_recovery_tree(&snapshot.join(index.to_string()), root);
    }
    assert_eq!(
        crate::UnifiedEndpoint::persisted_endpoint_id(&config.endpoint_data_dir)
            .expect("restored persisted endpoint"),
        endpoint
    );
    let (runtime, guard) = LocalProvisioner::open_runtime(config.clone())
        .await
        .expect("complete same-path recovery");
    assert_eq!(runtime.endpoint_id(), endpoint);
    assert!(
        runtime
            .node_public_ready()
            .await
            .expect("restored local readiness")
    );
    exercise_running_host(
        runtime,
        serde_json::from_value(journal["user"].clone()).expect("retained initial User"),
    )
    .await;
    drop(guard);
    let owner = idp_server::OwnerProvisioner::open(&config.idp)
        .await
        .expect("restored owner");
    assert!(
        owner
            .verify_node_signer(&binding)
            .await
            .expect("restored signing capability")
            == signer,
        "same trusted public signer"
    );
    drop(owner);
    let (mut runtime, guard) = LocalProvisioner::open_runtime(config.clone())
        .await
        .expect("restored Node reopens again");
    runtime
        .start_background_tasks()
        .await
        .expect("recovered runtime completes startup before shutdown");
    assert_eq!(runtime.endpoint_id(), endpoint);
    let client = runtime.management_control_plane.clone();
    assert_eq!(
        client
            .get_storage_endpoint_identity(&endpoint.to_string())
            .await
            .expect("authenticated restored Management owner trust")
            .endpoint_id,
        endpoint.to_string()
    );
    tokio::time::timeout(Duration::from_secs(60), runtime.shutdown())
        .await
        .expect("bounded recovered shutdown")
        .expect("recovered shutdown");
    drop(client);
    drop(guard);
    let restored_lock = fs::metadata(&lock).expect("restored original lock");
    assert_eq!(
        (lock_identity.dev(), lock_identity.ino()),
        (restored_lock.dev(), restored_lock.ino())
    );
    assert_eq!(fixture.journal(), journal, "retained recovery operation");
    assert!(
        store
            .update(|records| Ok(records.clone()))
            .expect("restored SQL")
            == logical,
        "restore includes the entire canonical operation and provisioning metadata"
    );
    assert_stopped_recovery_tree(
        &snapshot.join("0/local-client-key-references"),
        &roots[0].join("local-client-key-references"),
    );
    assert_matching_bytes(
        &fs::read(snapshot.join("0/setup-secret-inventory.json")).expect("snapshot inventory"),
        &fs::read(roots[0].join("setup-secret-inventory.json")).expect("retained inventory"),
        "retained secret inventory",
    );
    for (index, root) in roots.iter().enumerate().filter(|(index, _)| *index != 4) {
        let lost = fixture.root.join(format!("missing-restored-{index}"));
        fs::rename(root, &lost).expect("isolate missing restored context or endpoint");
        assert!(
            LocalProvisioner::open_runtime(config.clone())
                .await
                .is_err(),
            "missing restored root denies"
        );
        assert!(
            !root.exists(),
            "denial must not initialize a replacement root"
        );
        fs::rename(lost, root).expect("retain exact restored root");
    }
    let endpoint_key = roots[3].join("endpoint.key");
    fs::write(&endpoint_key, b"invalid-current-format-endpoint")
        .expect("corrupt only isolated restored endpoint");
    assert!(
        LocalProvisioner::open_runtime(config.clone())
            .await
            .is_err(),
        "invalid restored endpoint denies"
    );
    assert!(
        fs::read(&endpoint_key).expect("rejected endpoint retained")
            == b"invalid-current-format-endpoint"
    );
    fs::remove_file(&endpoint_key).expect("remove injected unsupported layout");
    store
        .put("create", &serde_json::json!({"version": 0}))
        .expect("corrupt restored SQL metadata");
    let corrupt = store
        .update(|records| Ok(records.clone()))
        .expect("corrupt logical snapshot");
    assert!(
        LocalProvisioner::open_runtime(config).await.is_err(),
        "present invalid SQL operation never falls back"
    );
    assert!(
        store
            .update(|records| Ok(records.clone()))
            .expect("rejected SQL retained")
            == corrupt,
        "refusal leaves all corrupt SQL evidence intact"
    );
    assert!(
        fixture.secrets() == secrets,
        "restore and denial never clone or replace protected material"
    );
}

#[tokio::test]
async fn host_references_drive_complete_owner_create_and_offline_reopen() {
    let fixture = Fixture::new();
    let unresolved = fixture.config.clone();

    let credentials = crate::CredentialBootstrap::create(unresolved.clone())
        .expect("explicit fresh public references");
    let config = credentials.load().expect("retained host settings");
    let mut host = LocalProvisioner::create(
        config.clone(),
        "operator",
        "Automatic Node",
        &fixture.password,
    )
    .expect("generated config works at public owner seam");
    host.provision(&fixture.password)
        .await
        .expect("complete actual Node create");
    let journal = fixture.journal();
    let user = journal["user"]
        .as_str()
        .expect("administrator ID")
        .parse()
        .expect("canonical ID");
    let runtime = host
        .build_runtime()
        .await
        .expect("generated credentials compose all contexts");
    let endpoint = runtime.endpoint_id();
    exercise_running_host(runtime, user).await;
    let retained_secrets = fixture.secrets();
    drop(host);
    drop(credentials);
    let credentials =
        crate::CredentialBootstrap::open(unresolved).expect("offline public credential reopen");
    let config = credentials.load().expect("existing-only host settings");
    let host = LocalProvisioner::open(config).expect("offline exact owner intent");
    let runtime = host
        .build_runtime()
        .await
        .expect("offline actual Node reopen");
    assert_eq!(runtime.endpoint_id(), endpoint);
    assert!(
        runtime
            .node_public_ready()
            .await
            .expect("current local readiness")
    );
    runtime.shutdown().await.expect("close all contexts");
    assert_eq!(
        retained_secrets,
        fixture.secrets(),
        "no regeneration on offline restart"
    );
}

#[tokio::test]
async fn join_intent_is_public_exact_and_never_startup_readiness() {
    let fixture = Fixture::new();
    let peer = iroh::SecretKey::generate().public().to_string();
    let local = crate::LocalJoin::create(fixture.config.clone(), &peer, "Joining Node")
        .expect("purpose-bound fresh join intent");
    assert!(
        local.build_runtime().await.is_err(),
        "intent is not completion"
    );
    assert!(
        crate::LocalJoin::open(fixture.config.clone()).is_err(),
        "cooperating join exclusion"
    );
    secret_files_absent(&fixture.root);
    let store = crate::host_store::HostStore::for_config(&fixture.config)
        .expect("Join uses the host SQL store");
    let before: serde_json::Value = store.get("join").expect("SQL read").expect("Join record");
    assert!(
        !fixture
            .config
            .local_provisioning_directory()
            .join("join.json")
            .exists(),
        "Unified Join must not publish a parallel journal"
    );
    drop(local);
    let local = crate::LocalJoin::open(fixture.config.clone()).expect("exact continuation");
    assert!(
        local.build_https_runtime().await.is_err(),
        "incomplete join cannot serve HTTPS"
    );
    drop(local);
    assert!(
        LocalProvisioner::open(fixture.config.clone()).is_err(),
        "join cannot be reinterpreted as create"
    );
    let mut changed = fixture.config.clone();
    changed.idp.oauth2.issuer = "https://wrong-installation.test/idp".into();
    assert!(crate::LocalJoin::open(changed).is_err());
    assert_eq!(
        store
            .get::<serde_json::Value>("join")
            .expect("SQL read")
            .expect("Join"),
        before
    );
    secret_files_absent(&fixture.root);
}

#[tokio::test]
async fn join_missing_or_corrupt_host_metadata_never_replaces_endpoint_or_allocations() {
    let fixture = Fixture::new();
    let peer = iroh::SecretKey::generate().public().to_string();
    let mut joining = crate::LocalJoin::create(fixture.config.clone(), &peer, "Joining Node")
        .expect("fresh Join");
    let endpoint = joining
        .prepare_endpoint()
        .await
        .expect("mock-provider endpoint");
    let expected = endpoint.endpoint_id();
    endpoint.close().await;
    drop(joining);
    let root = crate::BootstrapConfig::explicit_directory(&fixture.config);
    let database = root.join("host.redb");
    let backup = root.join("operator-backup.redb");
    fs::rename(&database, &backup).expect("simulate lost host metadata");
    let secrets = fixture.secrets();
    assert!(crate::LocalJoin::open(fixture.config.clone()).is_err());
    assert!(!database.exists(), "missing SQL must not be recreated");
    fs::write(&database, b"corrupt retained host SQL").expect("inject corruption");
    assert!(crate::LocalJoin::open(fixture.config.clone()).is_err());
    assert!(
        fs::read(&database).expect("retained corrupt artifact") == b"corrupt retained host SQL",
        "corrupt metadata was modified"
    );
    fs::remove_file(&database).expect("remove injected corruption");
    fs::rename(backup, database).expect("restore exact operator backup");
    let mut joining = crate::LocalJoin::open(fixture.config.clone()).expect("exact restored Join");
    let endpoint = joining
        .prepare_endpoint()
        .await
        .expect("existing endpoint only");
    assert_eq!(endpoint.endpoint_id(), expected);
    endpoint.close().await;
    assert!(
        fixture.secrets() == secrets,
        "provider state changed during failed reopen"
    );
}

#[tokio::test]
async fn local_provisioning_creates_exact_host_and_reopens_offline() {
    let fixture = Fixture::new();
    let mut host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Local Node",
        &fixture.password,
    )
    .expect("explicit local fresh intent");
    secret_files_absent(&fixture.root);
    assert!(
        host.build_runtime().await.is_err(),
        "incomplete setup is not startup readiness"
    );
    host.provision(&fixture.password)
        .await
        .expect("public local provisioning seam");
    let intent = fixture.journal();
    assert_eq!(
        intent["complete"], true,
        "completion follows actual local readiness"
    );
    let manifest = fixture.journal().to_string();
    for secret in ["fixture-initial-password"] {
        assert!(
            !manifest.contains(secret),
            "raw credentials must not enter retained intent"
        );
    }
    let secrets = fixture.secrets();
    let runtime = host
        .build_runtime()
        .await
        .expect("actual composed host startup");
    assert!(
        runtime
            .node_public_ready()
            .await
            .expect("local trust and context grants")
    );
    let endpoint = runtime.endpoint_id();
    exercise_running_host(
        runtime,
        intent["user"]
            .as_str()
            .expect("initial User")
            .parse()
            .expect("User ID"),
    )
    .await;
    drop(host);
    let host =
        LocalProvisioner::open(fixture.config.clone()).expect("existing-only journal reopen");
    let runtime = host
        .build_runtime()
        .await
        .expect("actual offline host reopen");
    assert_eq!(
        endpoint,
        runtime.endpoint_id(),
        "canonical endpoint survives restart"
    );
    assert!(
        runtime
            .node_public_ready()
            .await
            .expect("current readiness without a peer")
    );
    exercise_running_host(
        runtime,
        intent["user"]
            .as_str()
            .expect("initial User")
            .parse()
            .expect("User ID"),
    )
    .await;
    assert!(
        secrets == fixture.secrets(),
        "host startup/reopen must not produce replacement private material"
    );
    assert!(
        intent == fixture.journal(),
        "startup must preserve exact completed intent"
    );
    fixture
        .endpoint_entry()
        .delete_credential()
        .expect("simulate endpoint provider loss");
    assert!(
        host.build_runtime().await.is_err(),
        "completed startup never falls back to fresh identity"
    );
    assert!(
        !fixture.root.join("endpoint/endpoint.key").exists(),
        "missing identity stays missing"
    );
}

#[tokio::test]
async fn local_provisioning_continues_retained_attempt_and_refuses_changed_identity() {
    let mut fixture = Fixture::new();
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("occupied listener fixture");
    fixture.config.listen_addr = occupied.local_addr().expect("retained listener address");
    let mut host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Local Node",
        &fixture.password,
    )
    .expect("fresh intent");
    assert!(
        host.provision(&fixture.password).await.is_err(),
        "later context failure leaves setup incomplete"
    );
    let retained = fixture.journal();
    assert_eq!(retained["complete"], false);
    assert!(
        retained["public_signer"].is_object(),
        "exact signer retained before failed composition"
    );
    assert!(
        host.build_runtime().await.is_err(),
        "partial owner setup is not host completion"
    );
    assert!(
        LocalProvisioner::open(fixture.config.clone()).is_err(),
        "one local operation owns this host"
    );
    let secrets = fixture.secrets();
    drop(host);
    let mut changed = fixture.config.clone();
    changed.idp.oauth2.issuer = "https://changed-issuer.test".into();
    assert!(
        LocalProvisioner::open(changed).is_err(),
        "env credential drift never rotates retained Client"
    );
    let mut changed = fixture.config.clone();
    changed.idp.oauth2.issuer = "https://different-installation.test/idp".into();
    assert!(
        LocalProvisioner::open(changed).is_err(),
        "Canonical Issuer drift is not a new Installation"
    );
    assert!(
        LocalProvisioner::create(
            fixture.config.clone(),
            "operator",
            "Local Node",
            &fixture.password
        )
        .is_err(),
        "retained attempt cannot be redeclared fresh"
    );
    drop(occupied);
    let mut host =
        LocalProvisioner::open(fixture.config.clone()).expect("exact offline continuation");
    fs::write(&fixture.password, b"wrong-fixture-password\n").expect("wrong protected input");
    assert!(
        host.provision(&fixture.password).await.is_err(),
        "wrong password denies before owner writes"
    );
    assert!(retained == fixture.journal());
    fs::write(&fixture.password, b"fixture-initial-password\n")
        .expect("restore exact fixture password");
    host.provision(&fixture.password)
        .await
        .expect("continue original public-owner preparation and compose contexts");
    let completed = fixture.journal();
    for field in [
        "installation",
        "user",
        "credential",
        "member",
        "key",
        "role",
        "permissions",
        "intent",
        "endpoint",
        "public_signer",
    ] {
        assert!(
            retained[field] == completed[field],
            "continuation must retain each original identity/request"
        );
    }
    assert!(
        secrets == fixture.secrets(),
        "exact continuation never replaces retained private secrets"
    );
}

#[tokio::test]
async fn local_provisioning_lost_journal_refuses_even_with_ready_owner_state() {
    let fixture = Fixture::new();
    let mut host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Node",
        &fixture.password,
    )
    .expect("fresh intent");
    host.provision(&fixture.password)
        .await
        .expect("complete host");
    let secrets = fixture.secrets();
    drop(host);
    let directory = fixture.config.local_provisioning_directory();
    fs::rename(&directory, directory.with_extension("lost")).expect("simulate metadata loss");
    assert!(
        LocalProvisioner::open_runtime(fixture.config.clone())
            .await
            .is_err()
    );
    assert!(
        crate::UnifiedRuntime::build(fixture.config.clone())
            .await
            .is_err()
    );
    assert!(fixture.secrets() == secrets, "provider inventory changed");
    assert!(!directory.exists());
    assert!(
        !fixture
            .config
            .endpoint_data_dir
            .join("endpoint.key")
            .exists()
    );
}

#[tokio::test]
async fn local_provisioning_lost_management_after_late_failure_denies_without_recreation() {
    let mut fixture = Fixture::new();
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("occupied listener");
    fixture.config.listen_addr = occupied.local_addr().expect("listener address");
    let mut host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Local Node",
        &fixture.password,
    )
    .expect("fresh intent");
    assert!(host.provision(&fixture.password).await.is_err());
    let retained = fixture.journal();
    assert_eq!(retained["complete"], false);
    let database = fixture.root.join("device/devices.redb");
    assert!(
        database.is_file(),
        "real Management initialization precedes late failure"
    );
    let secrets = fixture.secrets();
    let database_before = fs::read(&database).expect("retained Management bytes");
    let mut unacknowledged = retained.clone();
    unacknowledged["management_complete"] = false.into();
    unacknowledged["storage_attempted"] = false.into();
    unacknowledged["storage_initialized"] = false.into();
    fixture.write_journal(&unacknowledged);
    let error = host
        .provision(&fixture.password)
        .await
        .expect_err("attempted unacknowledged authority requires explicit recovery");
    assert!(
        error.to_string().contains("unacknowledged Management"),
        "{error}"
    );
    assert!(
        database_before == fs::read(&database).expect("unchanged Management"),
        "Management database changed"
    );
    assert!(unacknowledged == fixture.journal());
    assert!(secrets == fixture.secrets());
    fixture.write_journal(&retained);
    fs::remove_file(&database).expect("lose attempted Management database");
    drop(occupied);
    assert!(
        host.provision(&fixture.password).await.is_err(),
        "lost authority requires recovery"
    );
    assert!(
        !database.exists(),
        "retry must not reconstruct grants or Device policy"
    );
    let root = database.parent().expect("retained Device database root");
    fs::remove_dir_all(root).expect("lose acknowledged Device root too");
    assert!(host.provision(&fixture.password).await.is_err());
    assert!(!root.exists(), "no replacement root");
    assert!(
        retained == fixture.journal(),
        "retain exact incomplete evidence"
    );
    assert!(secrets == fixture.secrets(), "no new private production");
}

#[tokio::test]
async fn local_provisioning_lost_storage_after_runtime_initialization_denies_without_recreation() {
    let fixture = Fixture::new();
    let mut host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Local Node",
        &fixture.password,
    )
    .expect("fresh intent");
    host.provision(&fixture.password)
        .await
        .expect("actual initialized Storage runtime");
    let mut retained = fixture.journal();
    retained["complete"] = false.into();
    fixture.write_journal(&retained);
    let mut unacknowledged = retained.clone();
    unacknowledged["storage_initialized"] = false.into();
    fixture.write_journal(&unacknowledged);
    assert!(
        host.provision(&fixture.password).await.is_err(),
        "unacknowledged Storage requires recovery even with retained roots"
    );
    assert!(unacknowledged == fixture.journal());
    fixture.write_journal(&retained);
    let root = fixture.root.join("storage");
    assert!(root.is_dir(), "real runtime initialized Storage");
    fs::rename(&root, fixture.root.join("lost-storage")).expect("lose retained Storage root");
    assert!(
        host.provision(&fixture.password).await.is_err(),
        "lost attempted Storage is not fresh"
    );
    assert!(!root.exists(), "no empty replacement context");
    assert!(retained == fixture.journal());
    fs::rename(fixture.root.join("lost-storage"), &root).expect("restore exact retained root");
    host.provision(&fixture.password)
        .await
        .expect("exact acknowledged Storage continuation");
    fs::remove_dir_all(root.join("databases")).expect("lose acknowledged Storage runtime root");
    assert!(
        host.build_runtime().await.is_err(),
        "acknowledged Storage root loss requires recovery"
    );
    assert!(
        !root.join("databases").exists(),
        "no runtime scaffold recreation after acknowledgement"
    );
}

#[tokio::test]
async fn local_provisioning_missing_attempted_signer_denies_without_replacement() {
    let mut fixture = Fixture::new();
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("occupied listener fixture");
    fixture.config.listen_addr = occupied.local_addr().expect("retained listener address");
    let mut host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Local Node",
        &fixture.password,
    )
    .expect("fresh intent");
    assert!(host.provision(&fixture.password).await.is_err());
    let journal = fixture.journal();
    let selector = format!(
        "node:{}:{}",
        journal["member"].as_str().expect("member ID"),
        journal["key"].as_str().expect("key ID")
    );
    let spec = HashMap::from([("user", selector.as_str())]);
    let entries = fixture
        .store
        .search(&spec)
        .expect("exact test-only Node credential selection");
    assert_eq!(entries.len(), 1, "one captured Node root");
    entries[0]
        .delete_credential()
        .expect("simulate attempted signer loss");
    let secrets = fixture.secrets();
    drop(host);
    let mut host = LocalProvisioner::open(fixture.config.clone()).expect("retained host intent");
    assert!(
        host.provision(&fixture.password).await.is_err(),
        "existing-only prepared verification refuses missing signer"
    );
    assert!(
        host.build_runtime().await.is_err(),
        "incomplete host remains unready"
    );
    assert!(
        secrets == fixture.secrets(),
        "no replacement provider writes"
    );
    assert!(
        journal == fixture.journal(),
        "failed continuation preserves exact retry/recovery evidence"
    );
}

#[tokio::test]
async fn local_provisioning_reuses_pre_enrollment_signer_after_readback_interruption() {
    let fixture = Fixture::new();
    let mut host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Local Node",
        &fixture.password,
    )
    .expect("fresh intent");
    fixture.fail_node_readback.store(true, Ordering::SeqCst);
    assert!(
        host.provision(&fixture.password).await.is_err(),
        "post-store error leaves captured preparation but no enrollment"
    );
    assert!(
        !fixture.fail_node_readback.load(Ordering::SeqCst),
        "actual Node store/readback boundary exercised"
    );
    let retained = fixture.journal();
    assert_eq!(retained["node_attempted"], true);
    assert_eq!(
        retained["management_attempted"], false,
        "Management phase has genuinely never started"
    );
    assert_eq!(retained["storage_attempted"], false);
    assert!(!fixture.root.join("management").exists());
    assert!(!fixture.root.join("storage").exists());
    assert!(
        retained["public_signer"].is_null(),
        "caller has not acknowledged public signer capture yet"
    );
    let snapshot = fixture.secrets();
    drop(host);
    let mut host =
        LocalProvisioner::open(fixture.config.clone()).expect("same durable local intent");
    host.provision(&fixture.password)
        .await
        .expect("existing-only prepared verification before exact enrollment");
    let current = fixture.journal();
    assert_eq!(current["complete"], true);
    assert_eq!(current["management_attempted"], true);
    assert_eq!(current["management_complete"], true);
    assert!(
        fixture.root.join("device/devices.redb").is_file(),
        "first authorized Management initialization succeeds"
    );
    for field in [
        "installation",
        "user",
        "credential",
        "member",
        "key",
        "role",
        "permissions",
        "intent",
        "endpoint",
    ] {
        assert!(
            retained[field] == current[field],
            "pre-enrollment continuation retains original IDs and requests"
        );
    }
    let current_secrets = fixture.secrets();
    assert!(
        snapshot
            .iter()
            .all(|secret| current_secrets.contains(secret)),
        "retained User/Node material is reused while untouched Client slots complete"
    );
}

#[test]
fn local_provisioning_rejects_fifo_password_before_opening_it() {
    let fixture = Fixture::new();
    let fifo = fixture.root.join("password-fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .args(["-m", "600"])
            .arg(&fifo)
            .status()
            .expect("native FIFO fixture")
            .success()
    );
    let config = fixture.config.clone();
    let input = fifo.clone();
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        send.send(LocalProvisioner::create(config, "operator", "Local Node", &input).is_err())
            .expect("send invalid-input decision");
    });
    let decision = receive.recv_timeout(Duration::from_secs(1));
    if matches!(decision, Err(std::sync::mpsc::RecvTimeoutError::Timeout)) {
        fs::OpenOptions::new()
            .write(true)
            .open(&fifo)
            .expect("unblock only the failing FIFO reader fixture");
    }
    worker.join().expect("invalid-input worker terminates");
    assert!(
        matches!(decision, Ok(true)),
        "nonregular password input must refuse without blocking on open"
    );
    secret_files_absent(&fixture.root);
}

#[test]
fn local_provisioning_rejects_unprotected_input_and_missing_or_conflicting_intent() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fixture = Fixture::new();
    assert!(
        LocalProvisioner::open(fixture.config.clone()).is_err(),
        "startup cannot synthesize fresh intent"
    );
    secret_files_absent(&fixture.root);
    fs::set_permissions(&fixture.password, fs::Permissions::from_mode(0o644))
        .expect("unsafe input fixture");
    assert!(
        LocalProvisioner::create(
            fixture.config.clone(),
            "operator",
            "Local Node",
            &fixture.password
        )
        .is_err()
    );
    assert!(
        !fixture.config.local_provisioning_directory().exists(),
        "unsafe input denies before intent publication"
    );
    fs::set_permissions(&fixture.password, fs::Permissions::from_mode(0o600))
        .expect("protect input");
    let link = fixture.root.join("password-link");
    symlink(&fixture.password, &link).expect("symlink input fixture");
    assert!(
        LocalProvisioner::create(fixture.config.clone(), "operator", "Local Node", &link).is_err()
    );
    let host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Local Node",
        &fixture.password,
    )
    .expect("explicit intent");
    secret_files_absent(&fixture.root);
    drop(host);
    let mut unknown_request = fixture.journal();
    unknown_request["intent"]["unrecognized_request_field"] = true.into();
    fixture.write_journal(&unknown_request);
    assert!(
        LocalProvisioner::open(fixture.config.clone()).is_err(),
        "unknown nested request fields must not be silently normalized or migrated"
    );
    fixture.write_journal(&serde_json::json!({}));
    assert!(LocalProvisioner::open(fixture.config.clone()).is_err());
    fixture
        .host_store()
        .update(|records| {
            records.remove("create");
            Ok(())
        })
        .expect("lost Create record");
    assert!(LocalProvisioner::open(fixture.config.clone()).is_err());
    assert!(
        LocalProvisioner::create(
            fixture.config.clone(),
            "operator",
            "Local Node",
            &fixture.password
        )
        .is_err(),
        "lost prior intent is not fresh authorization"
    );
    secret_files_absent(&fixture.root);
}

#[test]
fn concrete_host_settings_preserve_the_durable_intent_when_normalized() {
    let config = UnifiedConfig {
        listen_addr: "127.0.0.1:3000".parse().expect("loopback address"),
        endpoint_data_dir: "bootstrap-settings/endpoint".into(),
        idp: idp_server::AppConfig {
            data_dir: "bootstrap-settings/idp".into(),
            api_public_uri: "https://identity.test/idp".into(),
            service_audience: Some("identity-service".into()),
            key_namespace: "local-bootstrap".into(),
            oauth2: idp_service::oauth2::OAuth2Config {
                issuer: "https://installation.test/idp".into(),
                ..Default::default()
            },
            ..Default::default()
        },
        management: management_server::AppConfig::default(),
        device: device_server::AppConfig {
            data_dir: "bootstrap-settings/device".into(),
            api_public_uri: "https://control.test/device".into(),
            storage_audience: "storage-service".into(),
            ..Default::default()
        },
        storage: storage_server::AppConfig {
            data_dir: "bootstrap-settings/storage".into(),
            api_public_uri: "https://resources.test/storage".into(),
            ..Default::default()
        },
    };
    let mut retained = serde_json::to_value(
        crate::local_provisioning::intent(&config).expect("valid owner settings"),
    )
    .expect("durable intent");
    assert_eq!(retained["issuer"], "https://installation.test/idp");
    assert_eq!(retained["key_namespace"], "local-bootstrap");
    assert_eq!(
        retained["audiences"],
        serde_json::json!(["identity-service", "storage-service"])
    );
    assert_eq!(
        retained["applications"],
        serde_json::json!([
            "https://identity.test/idp",
            "https://control.test/device",
            "https://resources.test/storage"
        ])
    );
    let directory = std::env::current_dir().expect("current directory");
    retained["roots"] = serde_json::json!([
        directory.join("bootstrap-settings/idp"),
        directory.join("bootstrap-settings/device"),
        directory.join("bootstrap-settings/storage"),
        directory.join("bootstrap-settings/endpoint"),
    ]);
    let normalized =
        crate::local_provisioning::normalized_config(config).expect("normalize minimal settings");
    assert!(normalized.endpoint_data_dir.is_absolute());
    assert!(Path::new(&normalized.device.data_dir).is_absolute());
    assert!(Path::new(&normalized.storage.data_dir).is_absolute());
    assert_eq!(
        serde_json::to_value(
            crate::local_provisioning::intent(&normalized).expect("normalized intent")
        )
        .expect("intent JSON"),
        retained,
    );
}

#[test]
fn retained_create_sql_keeps_owner_evidence_and_exact_retry_checks() {
    let fixture = Fixture::new();
    let host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Local Node",
        &fixture.password,
    )
    .expect("fresh retained intent");
    let original = fixture.journal();
    let mut expected_fields = vec![
        "version",
        "intent",
        "installation",
        "user",
        "credential",
        "login_client",
        "member",
        "key",
        "role",
        "permissions",
        "password_verifier",
        "endpoint_attempted",
        "endpoint_secret",
        "endpoint",
        "inventory_attempted",
        "node_attempted",
        "public_signer",
        "management_attempted",
        "management_complete",
        "storage_attempted",
        "storage_initialized",
        "complete",
    ];
    expected_fields.sort();
    let mut actual_fields = original
        .as_object()
        .expect("journal object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    actual_fields.sort();
    assert_eq!(
        actual_fields, expected_fields,
        "owner intent must preserve retained field names"
    );
    assert!(original["password_verifier"].is_string());
    assert!(original["permissions"].is_array());
    drop(host);
    let host =
        LocalProvisioner::open(fixture.config.clone()).expect("reopen original journal schema");
    host.validate_retry("operator", "Local Node", "fixture-initial-password")
        .expect("exact credentials");
    assert!(
        host.validate_retry("operator", "Local Node", "wrong-password")
            .is_err()
    );
    assert_eq!(fixture.journal(), original);
    drop(host);
    for (field, value) in [
        ("permissions", serde_json::json!([])),
        ("role", original["user"].clone()),
        ("unrecognized_request_field", serde_json::json!(true)),
    ] {
        let mut malformed = original.clone();
        malformed[field] = value;
        fixture.write_journal(&malformed);
        assert!(LocalProvisioner::open(fixture.config.clone()).is_err());
        assert_eq!(fixture.journal(), malformed);
    }
    secret_files_absent(&fixture.root);
}

#[path = "endpoint_provider_tests.rs"]
mod endpoint_provider_tests;

#[test]
fn join_choices_have_one_canonical_sql_owner_for_standalone_and_hosted_operations() {
    for hosted in [false, true] {
        let fixture = Fixture::new();
        let peer = iroh::SecretKey::generate().public().to_string();
        let expected = idp_server::hosted_setup::JoinIntent {
            operation_nonce: Id::now_v7().simple().to_string(),
            node: "Joining Node".into(),
            peer: peer.clone(),
            issuer: fixture.config.idp.oauth2.issuer.clone(),
        }
        .canonical()
        .expect("canonical Join");
        if hosted {
            crate::host_store::HostStore::create_for_config(&fixture.config)
                .expect("host SQL")
                .put("operation", &expected)
                .expect("bound Join");
        }
        let local = crate::LocalJoin::create(fixture.config.clone(), &peer, "Joining Node")
            .expect("fresh Join");
        let store = crate::host_store::HostStore::for_config(&fixture.config).expect("host SQL");
        let canonical: String = store
            .get("operation")
            .expect("SQL read")
            .expect("canonical binding even standalone");
        if hosted {
            assert_eq!(canonical, expected, "preserve admitted nonce");
        }
        let idp_server::hosted_setup::SetupIntent::Join(mut operation) =
            idp_server::hosted_setup::SetupIntent::decode(&canonical).expect("Join binding")
        else {
            panic!("expected Join")
        };
        assert_eq!(operation.peer, peer);
        assert_eq!(operation.node, "Joining Node");
        assert_eq!(operation.issuer, fixture.config.idp.oauth2.issuer);
        let journal: serde_json::Value = store
            .get("join")
            .expect("SQL read")
            .expect("Join allocations");
        assert!(
            journal.get("peer").is_none() && journal.get("name").is_none(),
            "choices are not duplicated in allocation metadata"
        );
        local
            .validate_retry(&peer, "Joining Node")
            .expect("exact choices");
        assert!(local.validate_retry(&peer, "Different Node").is_err());
        drop(local);
        operation.issuer = "https://different.test/idp".into();
        let invalid = operation
            .canonical()
            .expect("different authenticated issuer");
        store
            .put("operation", &invalid)
            .expect("inject conflicting issuer");
        let secrets = fixture.secrets();
        assert!(crate::LocalJoin::open(fixture.config.clone()).is_err());
        assert_eq!(
            store
                .get::<String>("operation")
                .expect("SQL read")
                .expect("binding"),
            invalid
        );
        assert_eq!(
            store
                .get::<serde_json::Value>("join")
                .expect("SQL read")
                .expect("allocations"),
            journal
        );
        assert!(
            fixture.secrets() == secrets,
            "conflicting issuer performs no provider work"
        );
    }
}
