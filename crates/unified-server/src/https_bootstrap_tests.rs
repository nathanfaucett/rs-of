use std::{
    collections::HashMap,
    fs, io,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

use idp_server::local_provisioning::provisioning_stage;

use idp_model::model::Id;
use idp_service::repo::PrivateKeyKeyringRepo;

use model::contract::TokenResponse;
use tokio::task::JoinHandle;

use super::Fixture;
use crate::{LocalJoin, LocalProvisioner, LocalTls, UnifiedRuntime};

struct NodeStores {
    stores: [(String, Arc<keyring_core::CredentialStore>); 2],
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
        modifiers: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<keyring_core::Entry> {
        let (_, store) = self
            .stores
            .iter()
            .find(|(namespace, _)| user.starts_with(&format!("{namespace}:")))
            .ok_or(keyring_core::Error::NoEntry)?;
        keyring_core::api::CredentialStoreApi::build(&**store, service, user, modifiers)
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

#[path = "api_join_tests.rs"]
mod api_join_tests;

#[test]
fn production_https_create_join_and_offline_reopen_with_isolated_native_roots() {
    https_bootstrap(false, "0");
}

#[test]
fn hosted_join_cancellation_exact_retry_and_offline_reopen() {
    https_bootstrap(true, "0");
}

#[test]
fn production_api_join_interruption_preserves_private_host_intent() {
    https_bootstrap(false, "interrupted");
}

#[test]
fn production_api_join_completion_and_offline_reopen() {
    https_bootstrap(false, "offline");
}

#[test]
fn production_setup_status_tracks_current_local_revocation() {
    https_bootstrap(false, "readiness");
}

#[test]
fn production_api_join_complete_unretired_finalization() {
    https_bootstrap(false, "finalization");
}

fn https_bootstrap(hosted: bool, api_join: &str) {
    let root = std::env::temp_dir().join(format!("unified-https-bootstrap-{}", Id::now_v7()));
    let a = crate::config::test_config(&root.join("a"));
    let b = crate::config::test_config(&root.join("b"));
    let tls_a = LocalTls::create(&a).expect("isolated source CA");
    let tls_b = LocalTls::create(&b).expect("isolated joined CA");
    let bundle = root.join("native-roots.pem");
    let mut certificates = fs::read(tls_a.ca_path()).expect("source CA");
    certificates.extend(fs::read(tls_b.ca_path()).expect("joined CA"));
    fs::write(&bundle, certificates).expect("isolated native roots, not OS installation");
    let empty = root.join("empty-native-roots");
    fs::create_dir(&empty).expect("isolate native cert directory");
    let mut child = Command::new(std::env::current_exe().expect("current test executable"))
        .args([
            "--exact",
            "local_provisioning_tests::https_bootstrap_tests::production_https_bootstrap_child",
            "--ignored",
            "--nocapture",
        ])
        .env("RUST_MIN_STACK", "16777216")
        .env("SSL_CERT_FILE", &bundle)
        .env("SSL_CERT_DIR", &empty)
        .env("NO_PROXY", "127.0.0.1,::1,localhost")
        .env("OF_TEST_SOURCE_TLS", LocalTls::directory(&a))
        .env("OF_TEST_JOIN_TLS", LocalTls::directory(&b))
        .env("OF_TEST_HOSTED_JOIN", if hosted { "1" } else { "0" })
        .env("OF_TEST_API_JOIN", api_join)
        .stdin(Stdio::null())
        .spawn()
        .expect("isolated native-root test process");
    let deadline = Instant::now() + Duration::from_secs(300);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("bound stalled bootstrap test");
            child.wait().expect("reap bounded child");
            fs::remove_dir_all(&root).expect("isolated root cleanup");
            panic!("production HTTPS bootstrap test exceeded its deadline");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    fs::remove_dir_all(root).expect("isolated root cleanup");
    assert!(
        status.success(),
        "production HTTPS create/join/reopen child must succeed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "launched by the parent test with isolated native-root environment, never the real OS store"]
async fn production_https_bootstrap_child() {
    let tls_source =
        PathBuf::from(std::env::var_os("OF_TEST_SOURCE_TLS").expect("isolated parent source TLS"));
    let tls_join =
        PathBuf::from(std::env::var_os("OF_TEST_JOIN_TLS").expect("isolated parent joining TLS"));
    crate::endpoint::use_isolated_address_lookup();
    let mut a = Fixture::new();
    let mut b = Fixture::new();
    let automatic_host = crate::BootstrapConfig::create(
        &a.root.join("automatic-node"),
        crate::BootstrapPurpose::Create,
        None,
    )
    .expect("ordinary no-config create derives retained identity and settings");
    let mut unresolved = automatic_host.config();
    unresolved.idp.password.memory_mib = 1;
    unresolved.idp.password.iterations = 1;
    PrivateKeyKeyringRepo::new_with_store(&unresolved.idp.oauth2.issuer, a.store.clone());
    let credential_guard = crate::CredentialBootstrap::create(unresolved)
        .expect("ordinary no-config stable host references");
    a.config = credential_guard.load().expect("retained host settings");
    b.config.listen_addr = unused_address().await;
    b.config.idp.key_namespace = format!("https-joining-{}", Id::now_v7());
    let nodes: Arc<keyring_core::CredentialStore> = Arc::new(NodeStores {
        stores: [
            (a.config.idp.key_namespace.clone(), a.store.clone()),
            (b.config.idp.key_namespace.clone(), b.store.clone()),
        ],
    });
    let api_join = std::env::var("OF_TEST_API_JOIN").expect("parent API mode");
    let is_api_join = api_join != "0";
    let store = if is_api_join {
        api_join_tests::credential_store(nodes, b.store.clone())
    } else {
        nodes
    };
    PrivateKeyKeyringRepo::new_with_store(&a.config.idp.oauth2.issuer, store);
    fs::rename(tls_source, LocalTls::directory(&a.config))
        .expect("retain source TLS from isolated parent");
    fs::rename(tls_join, LocalTls::directory(&b.config))
        .expect("retain joining TLS from isolated parent");
    let tls_a = LocalTls::open(&a.config).expect("source TLS");
    let tls_b = LocalTls::open(&b.config).expect("joining TLS");
    for tls in [&tls_a, &tls_b] {
        let mut commands = 0;
        tls.test_install_os_ca(|command| {
            commands += 1;
            assert_eq!(command.get_program(), "trust");
            true
        })
        .expect("injected install command, isolated native root provider");
        assert_eq!(commands, 1);
    }
    if is_api_join {
        api_join_tests::from_source(&a, &mut b, &tls_a, tls_b).await;
        return;
    }
    let mut local =
        LocalProvisioner::create(a.config.clone(), "operator", "HTTPS source", &a.password)
            .expect("fresh create intent");
    let bad_config = crate::config::test_config(&a.root.join("untrusted"));
    let untrusted =
        LocalTls::create(&bad_config).expect("CA deliberately absent from native root file");
    assert!(
        provisioning_stage(|| local.provision_https(&a.password, &untrusted))
            .await
            .is_err(),
        "untrusted production HTTPS must not complete setup"
    );
    assert_eq!(a.journal()["complete"], false);
    assert_eq!(a.journal()["storage_initialized"], true);
    let released = tokio::net::TcpListener::bind(a.config.listen_addr)
        .await
        .expect("failed qualification closes the production listener");
    drop(released);
    provisioning_stage(|| local.provision_https(&a.password, &tls_a))
        .await
        .expect("complete production HTTPS qualification on exact retry");
    let journal = a.journal();
    assert_eq!(journal["complete"], true);
    let user: Id = journal["user"]
        .as_str()
        .expect("administrator")
        .parse()
        .expect("administrator ID");
    let login = local
        .login_client()
        .expect("bootstrap-created public login registration");
    let login_id = login.client_id.expect("retained public login Client ID");
    assert!(login.client_secret.is_none());
    assert_eq!(login.allowed_scopes, ["openid", "profile", "email"]);
    assert_eq!(
        local
            .login_client()
            .expect("stable public status")
            .client_id
            .as_deref(),
        Some(login_id.as_str())
    );
    let retained = fs::read_to_string(a.config.local_provisioning_directory().join("intent.json"))
        .expect("public bootstrap intent");
    assert!(!retained.contains("fixture-initial-password"));
    assert!(
        provisioning_stage(|| local.verify_https(&untrusted))
            .await
            .is_err(),
        "completed production verification still requires native-root HTTPS trust"
    );
    let released = tokio::net::TcpListener::bind(a.config.listen_addr)
        .await
        .expect("failed completed verification releases the listener");
    drop(released);
    provisioning_stage(|| local.verify_https(&tls_a))
        .await
        .expect("completed production verification uses current local readiness and trusted HTTPS");
    drop(local);
    let (runtime, guard) =
        provisioning_stage(|| LocalProvisioner::open_runtime_https(a.config.clone()))
            .await
            .expect("retained source HTTPS runtime");
    let source = HttpsHost::start(runtime, &tls_a).await;
    let http = LocalTls::verification_client().expect("native-only certificate-verifying client");
    let session = token(
        &http,
        &source.base,
        &[
            ("grant_type", "password"),
            ("client_id", &login_id),
            ("username", "operator"),
            ("password", "fixture-initial-password"),
            ("scope", "openid"),
        ],
    )
    .await;
    let approving = exchange(
        &http,
        &source.base,
        &login_id,
        &session.access_token.0,
        management_service::MANAGEMENT_APPLICATION_URI,
    )
    .await;
    let storage = exchange(
        &http,
        &source.base,
        &login_id,
        &session.access_token.0,
        "storage:filesystems",
    )
    .await;
    assert_eq!(
        http.get(format!("{}/device/node-joins/pending", source.base))
            .send()
            .await
            .expect("pending requires authentication")
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let acceptance_url = format!("{}/device/node-joins/acceptance", source.base);
    assert_eq!(
        http.get(&acceptance_url)
            .send()
            .await
            .expect("acceptance requires bearer")
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.put(&acceptance_url)
            .bearer_auth("forged-bearer")
            .json(&serde_json::json!({"accepting": true}))
            .send()
            .await
            .expect("forged acceptance request")
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    let acceptance: serde_json::Value = http
        .get(&acceptance_url)
        .bearer_auth(&approving)
        .send()
        .await
        .expect("authorized acceptance state")
        .json()
        .await
        .expect("acceptance JSON");
    assert_eq!(acceptance["accepting"], false);
    assert_eq!(
        http.put(&acceptance_url)
            .bearer_auth(&approving)
            .json(&serde_json::json!({"accepting": true}))
            .send()
            .await
            .expect("actual authenticated source acceptance route")
            .status(),
        reqwest::StatusCode::OK
    );
    let peer = source.endpoint.to_string();
    let discovery = tokio::spawn({
        let peer = peer.clone();
        async move { crate::discover_installation_issuer(&peer).await }
    });
    let learned = discovery.await.expect("issuer discovery task").expect("actual serving background answers authenticated issuer discovery without pending API polling");
    assert_eq!(learned, a.config.idp.oauth2.issuer);
    source.assert_offers_released().await;
    let pending = pending_http(&http, &source.base, &approving).await;
    assert!(
        pending
            .as_array()
            .expect("pending Node requests")
            .is_empty(),
        "issuer discovery does not enroll a Node"
    );
    rejected_enrollment_keeps_policy_absent(&http, &source, &approving, user, &journal).await;
    b.config.idp.oauth2.issuer = learned;

    let hosted = std::env::var("OF_TEST_HOSTED_JOIN").expect("parent mode") == "1";
    let mut joining = LocalJoin::create(b.config.clone(), &peer, "Production joining Node")
        .expect("fresh exact join intent");
    let listener = if hosted {
        Some(
            tokio::net::TcpListener::bind(b.config.listen_addr)
                .await
                .expect("host already owns its sole listener"),
        )
    } else {
        None
    };
    if hosted {
        let before = join_journal(&b);
        let cancellation = tokio_util::sync::CancellationToken::new();
        cancellation.cancel();
        let result = joining.prepare_hosted_runtime(true, &cancellation).await;
        assert_eq!(
            result.err().expect("cancel before mutation").kind(),
            io::ErrorKind::Interrupted
        );
        assert_eq!(join_journal(&b), before);
        assert!(!b.config.endpoint_data_dir.exists());
        assert!(!std::path::Path::new(&b.config.idp.data_dir).exists());
        assert!(joining.reopen_hosted_runtime(true).await.is_err());

        let cancellation = tokio_util::sync::CancellationToken::new();
        let cancel = cancellation.clone();
        let task = tokio::spawn(async move {
            let result = joining.prepare_hosted_runtime(true, &cancellation).await;
            (joining, result)
        });
        let member: Id = before["member"]
            .as_str()
            .expect("member")
            .parse()
            .expect("member ID");
        let pending = pending_for_member(&http, &source.base, &approving, member).await;
        let retained = join_journal(&b);
        let secrets = b.secrets();
        cancel.cancel();
        let (cancelled, result) = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("cancellation closes outstanding transport promptly")
            .expect("host task");
        assert_eq!(
            result.err().expect("cancel awaiting approval").kind(),
            io::ErrorKind::Interrupted
        );
        assert_eq!(join_journal(&b), retained);
        assert_eq!(retained["complete"], false);
        assert!(
            device_service::replica::DbSelectionPolicyRepo::new(source.devices.clone().client())
                .get(member, &user.to_string(), Id::nil())
                .await
                .expect("cancelled membership policy")
                .is_none()
        );
        assert!(!std::path::Path::new(&b.config.storage.data_dir).exists());
        source.assert_offers_released().await;
        assert_eq!(
            http.post(format!(
                "{}/device/node-joins/{}/approve",
                source.base,
                pending["offer_id"].as_str().expect("offer ID")
            ))
            .bearer_auth(&approving)
            .send()
            .await
            .expect("cancelled offer approval")
            .status(),
            reqwest::StatusCode::NOT_FOUND
        );
        drop(cancelled);
        let mut conflicting = b.config.clone();
        conflicting.idp.oauth2.issuer = "https://different-installation.test".into();
        assert!(
            LocalJoin::open(conflicting).is_err(),
            "issuer cannot change on retry"
        );
        assert_eq!(
            join_journal(&b),
            retained,
            "conflicts preserve exact intent"
        );
        joining = LocalJoin::open(b.config.clone()).expect("exact interrupted Join reopens");
        assert_eq!(
            b.secrets(),
            secrets,
            "reopen cannot replace independent credentials"
        );
        assert_eq!(join_journal(&b), retained);
    }
    let join_task = tokio::spawn(async move {
        if hosted {
            let runtime = joining
                .prepare_hosted_runtime(true, &tokio_util::sync::CancellationToken::new())
                .await?;
            Ok::<_, io::Error>((joining, tls_b, Some(runtime)))
        } else {
            joining.join(&tls_b).await?;
            Ok((joining, tls_b, None))
        }
    });
    let expected_member: Id = join_journal(&b)["member"]
        .as_str()
        .expect("joining member")
        .parse()
        .expect("member ID");
    let pending = pending_for_member(&http, &source.base, &approving, expected_member).await;
    let offer: Id = pending["offer_id"]
        .as_str()
        .expect("offer")
        .parse()
        .expect("offer ID");
    let member = expected_member;
    assert_eq!(
        join_journal(&b)["complete"],
        false,
        "an offer alone is not completed bootstrap"
    );
    let approve_url = format!("{}/device/node-joins/{offer}/approve", source.base);
    assert_eq!(
        http.post(&approve_url)
            .bearer_auth("forged-bearer")
            .send()
            .await
            .expect("forged HTTP approval")
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    assert!(
        device_service::replica::DbSelectionPolicyRepo::new(source.devices.clone().client())
            .get(member, &user.to_string(), Id::nil())
            .await
            .expect("policy lookup after forged approval")
            .is_none()
    );
    let response = http
        .post(&approve_url)
        .bearer_auth(&approving)
        .send()
        .await
        .expect("actual production HTTPS approval route");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let approval: device_service::hosted_join::NodeJoinApproval =
        response.json().await.expect("public signed approval");
    assert_eq!(approval.owner_subject, user);
    assert_eq!(approval.request.member_id, member);
    let policy =
        device_service::replica::DbSelectionPolicyRepo::new(source.devices.clone().client())
            .get(member, &user.to_string(), Id::nil())
            .await
            .expect("approved policy lookup")
            .expect("post-enrollment default policy");
    assert!(policy.admin_allowed);
    assert!(
        policy.application_id.is_none()
            && policy.selected_kind.is_none()
            && policy.selected_id.is_none()
    );
    let (mut joining, tls_b, hosted_runtime) = join_task
        .await
        .expect("managed joining task")
        .expect("successful production HTTPS join");
    if let Some(mut runtime) = hosted_runtime {
        assert_eq!(
            join_journal(&b)["complete"],
            false,
            "preparation is not completion"
        );
        assert!(joining.reopen_hosted_runtime(true).await.is_err());
        let cancellation = tokio_util::sync::CancellationToken::new();
        cancellation.cancel();
        assert_eq!(
            joining
                .complete_hosted(&runtime, &cancellation)
                .await
                .expect_err("cancel before completion")
                .kind(),
            io::ErrorKind::Interrupted
        );
        assert_eq!(join_journal(&b)["complete"], false);
        assert_eq!(
            runtime
                .take_listener()
                .expect_err("hosted preparation cannot own a second listener")
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_ne!(
            runtime.endpoint_id(),
            source.endpoint,
            "independent joining endpoint"
        );
        runtime
            .start_background_tasks()
            .await
            .expect("current owner readiness");
        let cancellation = runtime.cancellation_token();
        let shutdown = cancellation.clone();
        let router = runtime.router();
        let listener = tls_b.listener(listener.expect("retained host listener"));
        let serving = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
        });
        assert_eq!(
            http.get(format!(
                "https://{}/storage/filesystems",
                b.config.listen_addr
            ))
            .bearer_auth(&storage)
            .send()
            .await
            .expect("authenticated joined operation")
            .status(),
            reqwest::StatusCode::OK
        );
        joining
            .complete_hosted(&runtime, &tokio_util::sync::CancellationToken::new())
            .await
            .expect("verified local completion after hosted HTTPS use");
        cancellation.cancel();
        serving
            .await
            .expect("host listener task")
            .expect("host listener shutdown");
        runtime.shutdown().await.expect("hosted owner cleanup");
    }
    assert_eq!(join_journal(&b)["complete"], true);
    let joining_login = joining.login_client();
    assert!(joining_login.client_secret.is_none());
    assert_ne!(joining_login.client_id.as_deref(), Some(login_id.as_str()));
    let runtime = provisioning_stage(|| joining.build_https_runtime())
        .await
        .expect("completed production join runtime");
    let endpoint = runtime.endpoint_id();
    provisioning_stage(|| runtime.verify_https_bootstrap(&tls_b))
        .await
        .expect("current local owner relationships and Storage over HTTPS");
    let secrets = b.secrets();
    assert!(!secrets.is_empty());
    assert!(
        secrets.iter().all(|secret| !a.secrets().contains(secret)),
        "no copied private signers"
    );
    source.stop().await;
    drop(guard);
    drop(joining);
    let joining =
        LocalJoin::open(b.config.clone()).expect("exact offline join reopen with source stopped");
    assert_eq!(
        joining.login_client(),
        joining_login,
        "offline reopen retains the public login Client"
    );
    let listener = if hosted {
        Some(
            tokio::net::TcpListener::bind(b.config.listen_addr)
                .await
                .expect("reopened host owns listener before owners"),
        )
    } else {
        None
    };
    let runtime = if hosted {
        provisioning_stage(|| joining.reopen_hosted_runtime(true)).await
    } else {
        provisioning_stage(|| joining.build_https_runtime()).await
    }
    .expect("offline trusted public state and independent signer");
    assert_eq!(runtime.endpoint_id(), endpoint);
    let reopened = HttpsHost::start_on(runtime, &tls_b, listener).await;
    let response = http
        .get(format!("{}/storage/filesystems", reopened.base))
        .bearer_auth(&storage)
        .send()
        .await
        .expect("ordinary authenticated Storage read while source is offline");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let resources: serde_json::Value = response.json().await.expect("Storage resource catalog");
    assert!(resources.is_array());
    reopened.stop().await;
    assert_eq!(
        secrets,
        b.secrets(),
        "offline reopen never regenerates private material"
    );
    assert_eq!(join_journal(&b)["complete"], true);
    drop(joining);
    let mut joining = LocalJoin::open(b.config.clone()).expect("retain exact completed intent");
    provisioning_stage(|| joining.join(&tls_b)).await.expect("completed join continuation performs production HTTPS qualification with the approving peer offline");
}

async fn pending_http(http: &reqwest::Client, base: &str, bearer: &str) -> serde_json::Value {
    let response = http
        .get(format!("{base}/device/node-joins/pending"))
        .bearer_auth(bearer)
        .send()
        .await
        .expect("actual authenticated pending route");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response.json().await.expect("public pending request JSON")
}

async fn pending_for_member(
    http: &reqwest::Client,
    base: &str,
    bearer: &str,
    member: Id,
) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let requests = pending_http(http, base, bearer).await;
        if let Some(request) = requests
            .as_array()
            .expect("pending array")
            .iter()
            .find(|request| request["request"]["member_id"] == member.to_string())
        {
            return request.clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "authenticated Node offer becomes visible through serving route"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn rejected_enrollment_keeps_policy_absent(
    http: &reqwest::Client,
    source: &HttpsHost,
    bearer: &str,
    owner: Id,
    journal: &serde_json::Value,
) {
    let allowed = iroh_chain::EndpointIdStore::default();
    allowed.replace([source.endpoint]);
    let server = iroh_chain::Server::bind_with_secret_key(
        crate::endpoint::TestPreset,
        iroh::SecretKey::generate(),
        allowed,
    )
    .await
    .expect("independent rejected sender");
    let member = Id::now_v7();
    let mut signer: idp_model::contract::JwkPublic =
        serde_json::from_value(journal["public_signer"].clone()).expect("public signer template");
    signer.kid = Id::now_v7().to_string();
    signer.params = idp_model::contract::JwkPublicParameters::Ec {
        crv: "unsupported-curve".into(),
        x: "invalid-x".into(),
        y: "invalid-y".into(),
    };
    let request = serde_json::json!({
        "purpose": "of.equal-node-join.v1", "installation": journal["intent"]["issuer"],
        "member_id": member, "endpoint_id": server.endpoint().id().to_string(),
        "name": "Rejected enrollment", "public_signer": signer, "approving_peer": source.endpoint.to_string(),
    });
    let sender = tokio::spawn({
        let server = server.clone();
        let request = request.clone();
        let peer = source.endpoint;
        async move {
            server
                .send_pairing_offer(
                    peer,
                    &serde_json::to_vec(&request).expect("invalid enrollment request"),
                )
                .await
        }
    });
    let pending = pending_for_member(http, &source.base, bearer, member).await;
    let offer = pending["offer_id"].as_str().expect("rejected offer ID");
    let response = http
        .post(format!("{}/device/node-joins/{offer}/approve", source.base))
        .bearer_auth(bearer)
        .send()
        .await
        .expect("authenticated invalid enrollment route");
    assert_eq!(
        response.status(),
        reqwest::StatusCode::CONFLICT,
        "IdP exact signer validation rejects before Management mutation"
    );
    let policies =
        device_service::replica::DbSelectionPolicyRepo::new(source.devices.clone().client());
    assert!(
        policies
            .get(member, &owner.to_string(), Id::nil())
            .await
            .expect("policy lookup after enrollment rejection")
            .is_none()
    );
    assert!(
        !source.server.peers().contains(server.endpoint().id()),
        "rejected enrollment must not gain transport admission"
    );
    assert_eq!(
        http.post(format!("{}/device/node-joins/{offer}/reject", source.base))
            .bearer_auth(bearer)
            .send()
            .await
            .expect("authenticated rejection route")
            .status(),
        reqwest::StatusCode::NO_CONTENT
    );
    assert!(
        sender
            .await
            .expect("rejected sender task")
            .expect("purpose pairing rejection delivered")
            .is_empty()
    );
    assert!(
        policies
            .get(member, &owner.to_string(), Id::nil())
            .await
            .expect("policy absent after rejection")
            .is_none()
    );
    server.endpoint().close().await;
    source.assert_offers_released().await;
    for expire in [false, true] {
        let allowed = iroh_chain::EndpointIdStore::default();
        allowed.replace([source.endpoint]);
        let server = iroh_chain::Server::bind_with_secret_key(
            crate::endpoint::TestPreset,
            iroh::SecretKey::generate(),
            allowed,
        )
        .await
        .expect("independent acceptance-window sender");
        let mut request = request.clone();
        request["endpoint_id"] = server.endpoint().id().to_string().into();
        if expire {
            source
                .server
                .set_pairing_enabled_for(Duration::from_secs(10));
        }
        let sender = tokio::spawn({
            let server = server.clone();
            let request = request.clone();
            let peer = source.endpoint;
            async move {
                server
                    .send_pairing_offer(peer, &serde_json::to_vec(&request).expect("stale offer"))
                    .await
            }
        });
        let pending = pending_for_member(http, &source.base, bearer, member).await;
        let stale = pending["offer_id"].as_str().expect("stale offer ID");
        let legacy = format!("{}/device/devices/pairing-accepting", source.base);
        if expire {
            let mut acceptance = source.server.subscribe_pairing_acceptance();

            acceptance
                .wait_for(Option::is_none)
                .await
                .expect("existing natural acceptance expiry");
        } else {
            assert_eq!(
                http.put(&legacy)
                    .bearer_auth(bearer)
                    .json(&serde_json::json!({"accepting": false}))
                    .send()
                    .await
                    .expect("authenticated Device acceptance disable")
                    .status(),
                reqwest::StatusCode::OK
            );
        }
        assert_eq!(
            http.put(&legacy)
                .bearer_auth(bearer)
                .json(&serde_json::json!({"accepting": true}))
                .send()
                .await
                .expect("authenticated Device acceptance re-enable")
                .status(),
            reqwest::StatusCode::OK
        );
        assert!(
            pending_http(http, &source.base, bearer)
                .await
                .as_array()
                .expect("pending array")
                .is_empty(),
            "disable/re-enable and expiry cannot resurrect an earlier acceptance offer"
        );
        assert_eq!(
            http.post(format!("{}/device/node-joins/{stale}/approve", source.base))
                .bearer_auth(bearer)
                .send()
                .await
                .expect("stale approval route")
                .status(),
            reqwest::StatusCode::NOT_FOUND
        );
        assert!(
            policies
                .get(member, &owner.to_string(), Id::nil())
                .await
                .expect("stale policy lookup")
                .is_none()
        );
        assert!(!source.server.peers().contains(server.endpoint().id()));
        if let Ok(reply) = tokio::time::timeout(Duration::from_secs(5), sender)
            .await
            .expect("invalidated offer closes its response transport")
            .expect("stale sender task")
        {
            assert!(
                reply.is_empty(),
                "invalidated offer receives no signed approval"
            );
        }
        source.assert_offers_released().await;
        server.endpoint().close().await;
    }
}

async fn unused_address() -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve test port");
    listener.local_addr().expect("nonzero localhost port")
}
fn join_journal(fixture: &Fixture) -> serde_json::Value {
    serde_json::from_slice(
        &fs::read(
            fixture
                .config
                .local_provisioning_directory()
                .join("join.json"),
        )
        .expect("public joining intent"),
    )
    .expect("public joining JSON")
}
async fn token(http: &reqwest::Client, base: &str, form: &[(&str, &str)]) -> TokenResponse {
    let response = http
        .post(format!("{base}/idp/oauth2/token"))
        .form(form)
        .send()
        .await
        .expect("ordinary local OAuth HTTPS call");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response.json().await.expect("ordinary OAuth response")
}
async fn exchange(
    http: &reqwest::Client,
    base: &str,
    client: &str,
    subject: &str,
    audience: &str,
) -> String {
    let mut form = vec![
        (
            "grant_type",
            "urn:ietf:params:oauth:grant-type:token-exchange",
        ),
        ("client_id", client),
        ("subject_token", subject),
        (
            "subject_token_type",
            "urn:ietf:params:oauth:token-type:access_token",
        ),
        ("resource", audience),
    ];
    if audience == "storage:filesystems" {
        form.push((
            "authorization_details",
            r#"[{"type":"storage","actions":["read","write"]}]"#,
        ));
    }
    token(http, base, &form).await.access_token.0
}

struct HttpsHost {
    cancellation: tokio_util::sync::CancellationToken,
    listener: Option<JoinHandle<io::Result<()>>>,
    endpoint: iroh::EndpointId,
    server: iroh_chain::Server,
    devices: Arc<db::Database>,
    node_joins: Arc<device_server::NodeJoinSource>,
    base: String,
}
impl HttpsHost {
    async fn start(runtime: UnifiedRuntime, tls: &LocalTls) -> Self {
        Self::start_on(runtime, tls, None).await
    }

    async fn start_on(
        mut runtime: UnifiedRuntime,
        tls: &LocalTls,
        listener: Option<tokio::net::TcpListener>,
    ) -> Self {
        runtime
            .start_background_tasks()
            .await
            .expect("current public readiness and completed background startup");
        let base = LocalTls::api_base(runtime.local_addr()).expect("exact secure base");
        let cancellation = runtime.cancellation_token();
        let endpoint = runtime.endpoint_id();
        let server = runtime.full_node_test_server().clone();
        let devices = runtime.full_node_test_devices();
        let node_joins = runtime.node_join_source_for_test();
        let tls = tls.clone();
        let task = if let Some(listener) = listener {
            assert_eq!(
                runtime
                    .take_listener()
                    .expect_err("hosted owners must not bind a second listener")
                    .kind(),
                io::ErrorKind::AlreadyExists
            );
            let listener = tls.listener(listener);
            let router = runtime.router();
            let cancellation = cancellation.clone();
            tokio::spawn(async move {
                let result = axum::serve(listener, router)
                    .with_graceful_shutdown(cancellation.cancelled_owned())
                    .await;
                let cleanup = runtime.shutdown().await;
                result.and(cleanup)
            })
        } else {
            tokio::spawn(runtime.serve_https(tls))
        };
        let host = Self {
            cancellation,
            listener: Some(task),
            endpoint,
            server,
            devices,
            node_joins,
            base,
        };
        let http = LocalTls::verification_client().expect("native root client");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            if http
                .get(format!("{}/idp/health", host.base))
                .send()
                .await
                .is_ok_and(|response| response.status() == reqwest::StatusCode::OK)
            {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "actual serve_https production listener becomes healthy"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        for context in ["idp", "management", "storage"] {
            assert_eq!(
                http.get(format!("{}/{context}/health", host.base))
                    .send()
                    .await
                    .expect("trusted live production context")
                    .status(),
                reqwest::StatusCode::OK
            );
        }
        host
    }
    async fn assert_offers_released(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while self.node_joins.retained_offer_count().await != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("transport closure/window end releases pending and answered offers");
    }

    async fn stop(mut self) {
        self.cancellation.cancel();
        self.listener
            .take()
            .expect("owned serving task")
            .await
            .expect("joined production serving task")
            .expect("gracefully close listener and all owners");
    }
}
impl Drop for HttpsHost {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(listener) = &self.listener {
            listener.abort();
        }
    }
}

#[test]
fn unsupported_tls_address_refuses_before_any_retained_material() {
    let root = std::env::temp_dir().join(format!("unified-invalid-https-address-{}", Id::now_v7()));
    let mut config = crate::config::test_config(&root);
    config.listen_addr = "127.0.0.2:3000".parse().expect("unsupported loopback SAN");
    assert!(LocalTls::create(&config).is_err());
    assert!(crate::CredentialBootstrap::create(config).is_err());
    assert!(
        !root.exists(),
        "no directories, keys or intent before address rejection"
    );
}
