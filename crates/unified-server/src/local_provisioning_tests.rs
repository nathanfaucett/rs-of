use idp_server::bootstrap_host::BootstrapHost;

use std::{
    collections::HashMap,
    fs,
    io::Write,
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
        let _ = PrivateKeyKeyringRepo::new_with_store(&config.idp.oauth2.issuer, store.clone());
        Self {
            root,
            config,
            password,
            store,
            fail_node_readback,
        }
    }

    fn journal(&self) -> serde_json::Value {
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
    let bytes = fs::read(
        fixture
            .config
            .local_provisioning_directory()
            .join("intent.json"),
    )
    .expect("public intent");
    assert!(!String::from_utf8(bytes).expect("JSON").contains(password));
    secret_files_absent(&fixture.root);
}

#[test]
fn live_credential_guard_refuses_missing_or_changed_intent_before_provider_work() {
    let fixture = Fixture::new();
    let config = crate::config::test_config(&fixture.root);
    let path = config
        .credential_bootstrap_directory()
        .join("credentials.json");
    let credentials =
        crate::CredentialBootstrap::create(config).expect("fresh protected references only");
    let retained = fs::read(&path).expect("exact public references");
    let secrets = fixture.secrets();
    fs::remove_file(&path).expect("simulate lost references under live guard");
    assert!(
        credentials.load().is_err(),
        "absence cannot authorize replacement"
    );
    assert!(!path.exists(), "missing retained state stays missing");
    idp_server::local_provisioning::private_options(true)
        .open(&path)
        .expect("restore isolated checkpoint")
        .write_all(&retained)
        .expect("write checkpoint");
    fs::write(&path, b"invalid retained credential intent").expect("simulate retained corruption");
    assert!(
        credentials.load().is_err(),
        "changed references cannot be overwritten"
    );
    assert_eq!(
        fs::read(&path).expect("fault remains"),
        b"invalid retained credential intent"
    );
    assert_eq!(
        fixture.secrets(),
        secrets,
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
        let path = automatic
            .credential_bootstrap_directory()
            .join("credentials.json");
        let mut old: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("references")).expect("JSON");
        assert!(old.get("clients").is_none());
        assert_eq!(old["roots"].as_array().expect("roots").len(), 4);
        old["namespace"] = serde_json::json!(Id::now_v7());
        old["clients"] =
            serde_json::json!((0..old_count).map(|_| Id::now_v7()).collect::<Vec<_>>());
        old["attempted"] = serde_json::json!(vec![true; old_count]);
        old["audiences"] = serde_json::json!(vec![automatic.idp_audience(); old_count]);
        let bytes = serde_json::to_vec(&old).expect("old references");
        fs::write(&path, &bytes).expect("retain old relationship evidence");
        let secrets = fixture.secrets();
        assert!(crate::CredentialBootstrap::open(automatic).is_err());
        assert_eq!(fs::read(path).expect("unchanged evidence"), bytes);
        assert_eq!(fixture.secrets(), secrets);
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
            let path = fixture.config.local_provisioning_directory().join(name);
            let mut old: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).expect("intent")).expect("JSON");
            assert!(old[field].get("clients").is_none());
            assert_eq!(old[field]["roots"].as_array().expect("roots").len(), 4);
            old[field]["clients"] = serde_json::json!(
                (0..old_count)
                    .map(|_| {
                        idp_server::local_provisioning::login_registration(
                            &crate::UnifiedBootstrapHost::settings(&fixture.config),
                            Id::now_v7(),
                        )
                    })
                    .collect::<Vec<_>>()
            );
            old["secret_verifiers"] =
                serde_json::json!(vec!["retained-obsolete-verifier"; old_count]);
            let bytes = serde_json::to_vec(&old).expect("old intent");
            fs::write(&path, &bytes).expect("old relationship intent");
            let secrets = fixture.secrets();
            if joining {
                assert!(crate::LocalJoin::open(fixture.config.clone()).is_err());
            } else {
                assert!(LocalProvisioner::open(fixture.config.clone()).is_err());
            }
            assert_eq!(fs::read(path).expect("unchanged intent"), bytes);
            assert_eq!(fixture.secrets(), secrets);
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
    assert_eq!(fixture.secrets(), secrets);
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
    let oauth2 = fixture_oauth2(&config, owner.node_join_database());
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
        DbKeyRepo::new(Arc::clone(&engine)),
        PrivateKeyKeyringRepo::new(&config.oauth2.issuer).expect("retained fixture private keys"),
        config.key_namespace.clone(),
    ));
    OAuth2Service::new(
        DbApplicationRepo::new(Arc::clone(&engine)),
        DbClientRepo::new(Arc::clone(&engine), Arc::clone(&keys)),
        DbOAuth2AuthorizationCodeRepo::new(Arc::clone(&engine)),
        DbOAuth2RefreshTokenRepo::new(Arc::clone(&engine)),
        DbUserRepo::new(Arc::clone(&engine), config.password.clone()),
        DbOAuth2UserConsentRepo::new(engine),
        keys,
        config.oauth2.clone(),
    )
}

async fn rotation_login_client(
    fixture: &Fixture,
    grants: Vec<idp_model::contract::GrantType>,
    audiences: Vec<String>,
) -> idp_model::contract::ClientRegistration {
    let owner = idp_server::OwnerProvisioner::open(&fixture.config.idp)
        .await
        .expect("stopped fixture IdP owner");
    let mut request = idp_server::local_provisioning::login_registration(
        &crate::UnifiedBootstrapHost::settings(&fixture.config),
        Id::now_v7(),
    );
    request.allowed_grant_types = grants;
    request.allowed_audiences = audiences;
    fixture_oauth2(&fixture.config.idp, owner.node_join_database())
        .register_client(request)
        .await
        .expect("independent public login fixture")
}

async fn exercise_running_host(runtime: crate::UnifiedRuntime, user: Id) {
    let permissions =
        management_service::LocalPermissions::new(runtime.join_owner().node_join_database());
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
    } else {
        assert!(
            fs::read(source).expect("snapshot bytes")
                == fs::read(restored).expect("restored bytes"),
            "restored bytes match without disclosing private material"
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
    ];
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
        LocalProvisioner::open_runtime(config.clone())
            .await
            .is_err(),
        "missing recovery set denies"
    );
    assert!(
        roots.iter().all(|root| !root.exists()),
        "missing roots are not repaired"
    );
    let journal_root = roots.last().expect("journal root");
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
    let (runtime, guard) = LocalProvisioner::open_runtime(config.clone())
        .await
        .expect("restored Node reopens again");
    assert_eq!(runtime.endpoint_id(), endpoint);
    let client = runtime.management_control_plane.clone();
    let cancellation = runtime.cancellation_token();
    let task = tokio::spawn(runtime.serve());
    assert_eq!(
        client
            .get_storage_endpoint_identity(&endpoint.to_string())
            .await
            .expect("authenticated restored Management owner trust")
            .endpoint_id,
        endpoint.to_string()
    );
    cancellation.cancel();
    tokio::time::timeout(Duration::from_secs(60), task)
        .await
        .expect("bounded recovered shutdown")
        .expect("recovered task")
        .expect("recovered shutdown");
    drop(client);
    drop(guard);
    let restored_lock = fs::metadata(&lock).expect("restored original lock");
    assert_eq!(
        (lock_identity.dev(), lock_identity.ino()),
        (restored_lock.dev(), restored_lock.ino())
    );
    let journal_path = journal_root.join("intent.json");
    assert!(
        fs::read(&journal_path).expect("unchanged journal")
            == fs::read(snapshot.join("4/intent.json")).expect("snapshot journal")
    );
    assert_stopped_recovery_tree(
        &snapshot.join("0/local-client-key-references"),
        &roots[0].join("local-client-key-references"),
    );
    assert!(
        fs::read(snapshot.join("0/setup-secret-inventory.json")).expect("snapshot inventory")
            == fs::read(roots[0].join("setup-secret-inventory.json")).expect("retained inventory")
    );
    for (index, root) in roots.iter().take(4).enumerate() {
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
    fs::copy(snapshot.join("3/endpoint.key"), &endpoint_key)
        .expect("recover exact fixture endpoint");
    fs::write(&journal_path, b"invalid-current-format-journal")
        .expect("corrupt only restored journal");
    assert!(
        LocalProvisioner::open_runtime(config).await.is_err(),
        "present invalid journal never falls back"
    );
    assert!(
        fs::read(&journal_path).expect("rejected journal retained")
            == b"invalid-current-format-journal"
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
    let path = fixture
        .config
        .local_provisioning_directory()
        .join("join.json");
    let before = fs::read(&path).expect("retained public join intent");
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
    assert_eq!(fs::read(&path).expect("unchanged intent"), before);
    secret_files_absent(&fixture.root);
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
    let manifest = fs::read_to_string(
        fixture
            .config
            .local_provisioning_directory()
            .join("intent.json"),
    )
    .expect("public-only manifest");
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
    fs::remove_file(fixture.root.join("endpoint/endpoint.key"))
        .expect("simulate canonical identity loss");
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
async fn local_provisioning_existing_cli_host_without_journal_starts_and_reopens() {
    use std::os::unix::fs::PermissionsExt;

    use idp_service::oauth2::NodeSignerBinding;
    let mut fixture = Fixture::new();
    let mut host = LocalProvisioner::create(
        fixture.config.clone(),
        "operator",
        "Local Node",
        &fixture.password,
    )
    .expect("explicit public owner provisioning");
    host.provision(&fixture.password)
        .await
        .expect("actual current host");
    let retained = fixture.journal();
    fixture.config.idp.oauth2.local_signer = Some(NodeSignerBinding {
        endpoint_id: retained["endpoint"].as_str().expect("endpoint").to_owned(),
        member_id: retained["member"]
            .as_str()
            .expect("member")
            .parse()
            .expect("ID"),
        key_id: retained["key"].as_str().expect("key").parse().expect("ID"),
    });
    let user = retained["user"]
        .as_str()
        .expect("User")
        .parse()
        .expect("ID");
    let secrets = fixture.secrets();
    drop(host);
    let directory = fixture.config.local_provisioning_directory();
    let journal_path = directory.join("intent.json");
    let mut incomplete = retained.clone();
    incomplete["complete"] = false.into();
    fs::write(
        &journal_path,
        serde_json::to_vec(&incomplete).expect("incomplete intent"),
    )
    .expect("retained incomplete fixture");
    assert!(
        LocalProvisioner::open_runtime(fixture.config.clone())
            .await
            .is_err(),
        "present incomplete journal is authoritative despite ready owner state"
    );
    fs::write(&journal_path, b"{}").expect("malformed journal");
    assert!(
        LocalProvisioner::open_runtime(fixture.config.clone())
            .await
            .is_err(),
        "present malformed journal cannot fall through"
    );
    fs::remove_file(&journal_path).expect("lost journal within present directory");
    assert!(
        LocalProvisioner::open_runtime(fixture.config.clone())
            .await
            .is_err(),
        "present directory with missing intent cannot fall through"
    );
    fs::remove_dir_all(&directory).expect("host retains owner state, not a CLI journal");
    let storage_root = Path::new(&fixture.config.storage.data_dir);
    fs::set_permissions(storage_root, fs::Permissions::from_mode(0o755))
        .expect("retained Storage permissions from ordinary 0022 umask");
    let runtime = crate::UnifiedRuntime::build(fixture.config.clone())
        .await
        .expect("public runtime supports retained 0755 Storage");
    assert!(
        runtime
            .node_public_ready()
            .await
            .expect("retained public readiness")
    );
    runtime.shutdown().await.expect("retained runtime shutdown");
    for _ in 0..2 {
        let (runtime, guard) = LocalProvisioner::open_runtime(fixture.config.clone())
            .await
            .expect("CLI existing-only decision accepts current explicit evidence");
        assert!(guard.is_none(), "no synthesized journal guard");
        assert!(
            runtime
                .node_public_ready()
                .await
                .expect("current readiness")
        );
        exercise_running_host(runtime, user).await;
        assert!(!directory.exists(), "no journal migration or synthesis");
        assert!(secrets == fixture.secrets(), "no private replacement");
        assert_eq!(
            fs::metadata(storage_root)
                .expect("retained Storage root")
                .permissions()
                .mode()
                & 0o777,
            0o755,
            "existing startup must not chmod retained Storage"
        );
    }
    // The desktop's public composition seam remains independent of CLI intent.
    let runtime = crate::UnifiedRuntime::build(fixture.config.clone())
        .await
        .expect("desktop composition remains public");
    assert!(
        runtime
            .node_public_ready()
            .await
            .expect("preprovisioned readiness")
    );
    runtime.shutdown().await.expect("joined shutdown");
    let endpoint = fs::read(fixture.root.join("endpoint/endpoint.key")).expect("retained endpoint");

    let management_before =
        fs::read(fixture.root.join("device/devices.redb")).expect("retained Management database");
    let mut unbound = fixture.config.clone();
    unbound.idp.oauth2.local_signer = None;
    assert!(
        LocalProvisioner::open_runtime(unbound).await.is_err(),
        "missing explicit signer is never inferred"
    );
    let mut untrusted = fixture.config.clone();
    untrusted
        .idp
        .oauth2
        .local_signer
        .as_mut()
        .expect("binding")
        .key_id = Id::now_v7();
    assert!(
        LocalProvisioner::open_runtime(untrusted.clone())
            .await
            .is_err(),
        "missing trust cannot provision a new signer"
    );
    assert_eq!(
        endpoint,
        fs::read(fixture.root.join("endpoint/endpoint.key")).expect("unchanged endpoint")
    );
    assert!(secrets == fixture.secrets());
    let owner = idp_server::OwnerProvisioner::open(&fixture.config.idp)
        .await
        .expect("retained owner");
    let public = owner
        .verify_node_signer(
            fixture
                .config
                .idp
                .oauth2
                .local_signer
                .as_ref()
                .expect("binding"),
        )
        .await
        .expect("original approved signer survives denied startup");
    assert_eq!(
        serde_json::to_value(public).expect("public signer"),
        retained["public_signer"]
    );
    assert!(
        owner
            .verify_node_signer(
                untrusted
                    .idp
                    .oauth2
                    .local_signer
                    .as_ref()
                    .expect("missing binding")
            )
            .await
            .is_err()
    );
    drop(owner);
    assert_eq!(
        management_before,
        fs::read(fixture.root.join("device/devices.redb"))
            .expect("Management unchanged on missing trust")
    );
    let endpoint_path = fixture.root.join("endpoint/endpoint.key");
    fs::set_permissions(&endpoint_path, fs::Permissions::from_mode(0o644))
        .expect("unprotected endpoint fixture");
    assert!(
        LocalProvisioner::open_runtime(fixture.config.clone())
            .await
            .is_err(),
        "retained directory policy must not weaken endpoint key security"
    );
    assert_eq!(
        fs::metadata(&endpoint_path)
            .expect("retained endpoint")
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
    fs::remove_file(endpoint_path).expect("lost endpoint");
    assert!(
        LocalProvisioner::open_runtime(fixture.config.clone())
            .await
            .is_err()
    );
    assert!(!fixture.root.join("endpoint/endpoint.key").exists());
    assert!(!directory.exists());
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
    let journal_path = fixture
        .config
        .local_provisioning_directory()
        .join("intent.json");
    fs::write(
        &journal_path,
        serde_json::to_vec(&unacknowledged).expect("unacknowledged Management intent"),
    )
    .expect("simulate interruption before Management acknowledgement");
    let error = host
        .provision(&fixture.password)
        .await
        .expect_err("attempted unacknowledged authority requires explicit recovery");
    assert!(
        error.to_string().contains("unacknowledged Management"),
        "{error}"
    );
    assert_eq!(
        database_before,
        fs::read(&database).expect("unchanged Management")
    );
    assert!(unacknowledged == fixture.journal());
    assert!(secrets == fixture.secrets());
    fs::write(
        &journal_path,
        serde_json::to_vec(&retained).expect("acknowledged original intent"),
    )
    .expect("restore original retained acknowledgement");
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
    fs::remove_dir_all(fixture.root.join("management")).expect("lose Management root too");
    assert!(host.provision(&fixture.password).await.is_err());
    assert!(
        !fixture.root.join("management").exists(),
        "no replacement root"
    );
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
    let journal_path = fixture
        .config
        .local_provisioning_directory()
        .join("intent.json");
    fs::write(
        &journal_path,
        serde_json::to_vec(&retained).expect("incomplete intent"),
    )
    .expect("interruption before final completion acknowledgement");
    let mut unacknowledged = retained.clone();
    unacknowledged["storage_initialized"] = false.into();
    fs::write(
        &journal_path,
        serde_json::to_vec(&unacknowledged).expect("unacknowledged Storage"),
    )
    .expect("interruption before Storage acknowledgement");
    assert!(
        host.provision(&fixture.password).await.is_err(),
        "unacknowledged Storage requires recovery even with retained roots"
    );
    assert!(unacknowledged == fixture.journal());
    fs::write(
        &journal_path,
        serde_json::to_vec(&retained).expect("acknowledged Storage"),
    )
    .expect("restore retained acknowledgement");
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
    let journal = fixture
        .config
        .local_provisioning_directory()
        .join("intent.json");
    let mut unknown_request = fixture.journal();
    unknown_request["intent"]["unrecognized_request_field"] = true.into();
    fs::write(
        &journal,
        serde_json::to_vec(&unknown_request).expect("unknown request fixture"),
    )
    .expect("write unknown request field");
    assert!(
        LocalProvisioner::open(fixture.config.clone()).is_err(),
        "unknown nested request fields must not be silently normalized or migrated"
    );
    fs::write(&journal, b"{}").expect("corrupt intent fixture");
    assert!(LocalProvisioner::open(fixture.config.clone()).is_err());
    fs::remove_file(journal).expect("lost intent fixture");
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

#[path = "full_node_tests.rs"]
mod full_nodes;
