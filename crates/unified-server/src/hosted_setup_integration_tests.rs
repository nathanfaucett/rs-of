use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use idp_model::model::Id;
use idp_service::repo::PrivateKeyKeyringRepo;
use tokio_util::sync::CancellationToken;

use crate::{BootstrapConfig, LocalTls, SetupHost, UnifiedConfig};

pub(crate) fn fixture_config(
    root: &std::path::Path,
    address: std::net::SocketAddr,
) -> UnifiedConfig {
    let mut config = crate::config::test_config(root);
    config.listen_addr = address;
    let base = format!("https://{address}");
    config.idp.ui_public_uri = base.clone();
    config.idp.api_public_uri = format!("{base}/idp");
    config.management.api_public_uri = format!("{base}/management");
    config.storage.api_public_uri = format!("{base}/storage");
    config.device.storage_audience = config.storage.api_public_uri.clone();
    config.idp.password.memory_mib = 1;
    config.idp.password.iterations = 1;
    config
}

#[tokio::test]
async fn retired_join_reopen_refuses_mismatched_or_missing_intent_without_owner_mutation() {
    use std::{io::Write, sync::Arc};

    use axum::{
        body::{Body, to_bytes},
        http::{HeaderMap, Request, StatusCode},
    };
    use tower::ServiceExt;

    for fault in [
        "different_retired_intent",
        "missing_checkpoint",
        "missing_owners",
        "missing_history",
        "wrong_host",
    ] {
        let root = std::env::temp_dir().join(format!("join-retirement-{}", Id::now_v7()));
        let mut host =
            BootstrapConfig::create(&root, crate::BootstrapPurpose::AwaitingChoice, None)
                .expect("isolated neutral host");
        let config = host.config();
        host.begin_setup().expect("neutral production history");
        let tls = LocalTls::create(&config).expect("isolated TLS, no trust installation");
        let admission = Arc::new(
            api::SetupAdmission::open(&root.join("setup-admission")).expect("protected capability"),
        );
        let state = root.join("setup-admission/state");
        let initial = fs::read_to_string(&state).expect("protected retrieval");
        let code = initial.lines().nth(1).expect("code");
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Setup {code}").parse().expect("header"),
        );
        let peer = iroh::SecretKey::generate().public().to_string();
        let issuer = "https://retained-installation.test/idp";
        let canonical = serde_json::to_string(&(
            "join",
            ["idp", "management", "storage"],
            "0123456789abcdef0123456789abcdef",
            "Retained Node",
            &peer,
            issuer,
        ))
        .expect("known canonical Join intent");
        if fault != "missing_checkpoint" {
            idp_server::local_provisioning::private_options(true)
                .open(root.join("join-admission.json"))
                .expect("protected exact host checkpoint")
                .write_all(canonical.as_bytes())
                .expect("write checkpoint");
        }
        if fault != "missing_history" {
            for stage in ["credentials", "owner"] {
                idp_server::local_provisioning::private_options(true)
                    .open(root.join(format!("join-{stage}.attempted")))
                    .expect("protected completed production history")
                    .write_all(b"attempted\n")
                    .expect("write production history");
            }
        }
        let retired = if fault == "different_retired_intent" {
            canonical.replace(
                "0123456789abcdef0123456789abcdef",
                "ffffffffffffffffffffffffffffffff",
            )
        } else {
            canonical.clone()
        };
        admission
            .bind(&headers, &retired)
            .expect("durable admission");
        admission
            .retire(&headers, &retired)
            .expect("retire selected operation");
        let host_peer = if fault == "wrong_host" {
            iroh::SecretKey::generate().public().to_string()
        } else {
            peer
        };
        host.select_join(&host_peer, issuer)
            .expect("retained host purpose");
        let retained = fs::read(&state).expect("retired bytes");
        let shutdown = CancellationToken::new();
        let coordinator =
            crate::setup_runtime::setup_worker(host, tls, admission, shutdown.clone());
        let router = coordinator.router().expect("production setup API");
        let owner = tokio::spawn(idp_server::local_provisioning::provisioning_stage(|| {
            coordinator.run()
        }));
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let response = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/idp/setup/status")
                            .body(Body::empty())
                            .expect("status request"),
                    )
                    .await
                    .expect("setup router");
                if response.status() == StatusCode::SERVICE_UNAVAILABLE {
                    let body: serde_json::Value = serde_json::from_slice(
                        &to_bytes(response.into_body(), 1024)
                            .await
                            .expect("status body"),
                    )
                    .expect("status JSON");
                    assert_eq!(body["ready"], false);
                    assert_eq!(body["state"], "unavailable");
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("invalid retired state fails before owner opening");
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/storage/filesystems")
                    .header("Authorization", "Bearer unrelated")
                    .body(Body::empty())
                    .expect("normal request"),
            )
            .await
            .expect("normal fallback");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(fs::read(&state).expect("unchanged fault"), retained);
        assert!(!std::path::Path::new(&config.idp.data_dir).exists());
        assert!(!config.endpoint_data_dir.exists());
        assert!(!config.credential_bootstrap_directory().exists());
        assert!(!config.local_provisioning_directory().exists());
        assert_eq!(
            root.join("join-admission.json").exists(),
            fault != "missing_checkpoint"
        );
        shutdown.cancel();
        owner
            .await
            .expect("host task")
            .expect("coordinated cleanup");
        drop(router);
        fs::remove_dir_all(root).expect("isolated cleanup");
    }
}

#[tokio::test]
async fn partial_credentials_reopen_preserves_recovery_for_create_and_join() {
    use std::{io::Write, sync::Arc};

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    for joining in [false, true] {
        for fault in [
            "valid",
            "malformed",
            "missing",
            "missing_root",
            "missing_owner",
            "unexpected_owner",
        ] {
            if !joining && matches!(fault, "missing_root" | "missing_owner" | "unexpected_owner") {
                continue;
            }
            let root = std::env::temp_dir().join(format!("partial-credentials-{}", Id::now_v7()));
            let mut host =
                BootstrapConfig::create(&root, crate::BootstrapPurpose::AwaitingChoice, None)
                    .expect("neutral host");
            host.begin_setup().expect("neutral setup production");
            let tls = LocalTls::create(&host.config()).expect("isolated neutral TLS");
            let admission = Arc::new(
                api::SetupAdmission::open(&root.join("setup-admission"))
                    .expect("protected setup admission"),
            );
            let code = fs::read_to_string(root.join("setup-admission/state"))
                .expect("protected retrieval")
                .lines()
                .nth(1)
                .expect("code")
                .to_owned();
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                "Authorization",
                format!("Setup {code}").parse().expect("setup header"),
            );
            let nonce = "0123456789abcdef0123456789abcdef";
            let (canonical, expected) = if joining {
                let peer = iroh::SecretKey::generate().public().to_string();
                let issuer = "https://partial-installation.test/idp";
                let canonical = serde_json::to_string(&(
                    "join",
                    ["idp", "management", "storage"],
                    nonce,
                    "Partial Node",
                    &peer,
                    issuer,
                ))
                .expect("Join intent");
                idp_server::local_provisioning::private_options(true)
                    .open(root.join("join-admission.json"))
                    .expect("Join checkpoint")
                    .write_all(canonical.as_bytes())
                    .expect("write intent");
                for (stage, state) in [
                    ("credentials", b"attempted\n".as_slice()),
                    ("owner", b"unattempted\n".as_slice()),
                ] {
                    idp_server::local_provisioning::private_options(true)
                        .open(root.join(format!("join-{stage}.attempted")))
                        .expect("production history")
                        .write_all(state)
                        .expect("write history");
                }
                host.select_join(&peer, issuer).expect("select exact Join");
                (
                    canonical,
                    serde_json::json!({"operation_nonce": nonce, "node": "Partial Node", "peer": peer, "issuer": issuer}),
                )
            } else {
                host.select_create().expect("select Create");
                (
                    serde_json::to_string(&(
                        "create",
                        ["idp", "management", "storage"],
                        nonce,
                        "operator",
                        "Partial Node",
                    ))
                    .expect("Create intent"),
                    serde_json::json!({"operation_nonce": nonce, "administrator": "operator", "node": "Partial Node"}),
                )
            };
            admission
                .bind(&headers, &canonical)
                .expect("durable exact admission");
            let config = host.config();
            drop(
                crate::CredentialBootstrap::adopt_host_tls(config.clone(), &tls)
                    .expect("valid partial credential references; no provider production"),
            );
            let references = config
                .credential_bootstrap_directory()
                .join("credentials.json");
            let retained = fs::read(&references).expect("public references");
            match fault {
                "malformed" => {
                    fs::write(&references, b"corrupt retained intent").expect("isolated corruption")
                }
                "missing" => {
                    fs::remove_file(&references).expect("isolated attempted reference loss")
                }
                "missing_root" => fs::remove_dir_all(config.credential_bootstrap_directory())
                    .expect("isolated attempted credential-root loss"),
                "missing_owner" => fs::write(root.join("join-owner.attempted"), b"attempted\n")
                    .expect("isolated missing attempted owner root"),
                "unexpected_owner" => idp_server::local_provisioning::create_private_directory(
                    &config.local_provisioning_directory(),
                    false,
                )
                .expect("isolated unacknowledged owner root"),
                _ => (),
            }
            let shutdown = CancellationToken::new();
            let mut coordinator =
                crate::setup_runtime::setup_worker(host, tls, admission, shutdown.clone());
            let router = coordinator.router().expect("production setup API");
            tokio::time::timeout(Duration::from_secs(5), coordinator.initialize())
                .await
                .expect("bounded partial credential reopen");
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/idp/setup/status")
                        .body(Body::empty())
                        .expect("status request"),
                )
                .await
                .expect("setup response");
            let status = response.status();
            let body: serde_json::Value = serde_json::from_slice(
                &to_bytes(response.into_body(), 2048)
                    .await
                    .expect("status body"),
            )
            .expect("status JSON");
            assert_eq!(body["ready"], false);
            if fault == "valid" {
                assert_eq!(status, StatusCode::OK);
                assert_eq!(body["state"], "unfinished");
                assert!(
                    crate::CredentialBootstrap::open(config.clone()).is_err(),
                    "initialized host retains the credential guard until authorized continuation or shutdown"
                );
            } else {
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
                assert_eq!(body["state"], "unavailable");
            }
            let recovery = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/idp/setup/intent")
                        .header("Authorization", format!("Setup {code}"))
                        .body(Body::empty())
                        .expect("authorized recovery"),
                )
                .await
                .expect("intent response");
            assert_eq!(recovery.status(), StatusCode::OK);
            let recovered: serde_json::Value = serde_json::from_slice(
                &to_bytes(recovery.into_body(), 2048)
                    .await
                    .expect("intent body"),
            )
            .expect("intent JSON");
            assert_eq!(recovered, expected);
            let normal = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/storage/filesystems")
                        .body(Body::empty())
                        .expect("normal request"),
                )
                .await
                .expect("normal response");
            assert_eq!(normal.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(
                config.local_provisioning_directory().exists(),
                fault == "unexpected_owner"
            );
            assert!(!config.endpoint_data_dir.exists());
            match fault {
                "missing" | "missing_root" => assert!(!references.exists()),
                "malformed" => assert_eq!(
                    fs::read(&references).expect("fault retained"),
                    b"corrupt retained intent"
                ),
                _ => assert_eq!(
                    fs::read(&references).expect("references retained"),
                    retained
                ),
            }
            shutdown.cancel();
            drop(coordinator);
            drop(router);
            fs::remove_dir_all(root).expect("isolated cleanup");
        }
    }
}

#[test]
fn production_api_create_authentication_and_offline_reopen() {
    let root = std::env::temp_dir().join(format!("api-create-{}", Id::now_v7()));
    fs::create_dir(&root).expect("isolated root");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve fixture address");
    let address = listener.local_addr().expect("fixture address");
    drop(listener);
    let config = fixture_config(&root, address);
    fs::write(root.join("fixture-address"), address.to_string()).expect("public fixture address");
    let host = BootstrapConfig::create_explicit(config).expect("neutral explicit host");
    let setup = SetupHost::create(host).expect("neutral TLS");
    let bundle = setup.tls().ca_path();
    drop(setup);
    let empty = root.join("empty-native-roots");
    fs::create_dir(&empty).expect("isolated native cert directory");
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "hosted_setup_integration_tests::api_create_child",
            "--ignored",
            "--nocapture",
        ])
        .env("OF_API_CREATE_ROOT", &root)
        .env("SSL_CERT_FILE", bundle)
        .env("SSL_CERT_DIR", empty)
        .env("RUST_MIN_STACK", "2097152")
        .stdin(Stdio::null())
        .spawn()
        .expect("isolated native roots child");
    let deadline = Instant::now() + Duration::from_secs(90);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill bounded child");
            child.wait().expect("reap child");
            panic!("narrow API Create child exceeded 90 seconds");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    fs::remove_dir_all(root).expect("isolated cleanup");
    assert!(status.success(), "actual API Create child must succeed");
}

struct SetupTimingLog;

impl log::Log for SetupTimingLog {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() == log::Level::Info
            && (metadata.target() == "unified_server::local_provisioning"
                || metadata.target() == "idp_server::hosted_setup")
            || metadata.level() == log::Level::Debug
                && [
                    "idp_server::independent",
                    "device_server::independent",
                    "storage_server::independent",
                    "storage_server::idp_client",
                    "device_server::router::routes::replication",
                ]
                .iter()
                .any(|target| metadata.target().starts_with(*target))
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("{}", record.args());
        }
    }

    fn flush(&self) {}
}

static SETUP_TIMING_LOG: SetupTimingLog = SetupTimingLog;

pub(super) fn capture_setup_timings() {
    log::set_logger(&SETUP_TIMING_LOG).expect("isolated timing logger");
    log::set_max_level(log::LevelFilter::Debug);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "parent supplies isolated native roots and mock provider; no real trust/provider updates"]
async fn api_create_child() {
    capture_setup_timings();
    crate::endpoint::use_isolated_address_lookup();
    let root = PathBuf::from(std::env::var_os("OF_API_CREATE_ROOT").expect("parent root"));
    PrivateKeyKeyringRepo::new_with_store(
        "isolated-api-create",
        std::sync::Arc::new(crate::local_provisioning_tests::SecretStore {
            store: keyring_core::mock::Store::new().expect("isolated provider"),
            fail_node_readback: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }),
    );
    let address = fs::read_to_string(root.join("fixture-address"))
        .expect("public fixture address")
        .parse()
        .expect("address");
    let config = fixture_config(&root, address);
    let host = BootstrapConfig::open_explicit(config.clone()).expect("retained host");
    let setup = SetupHost::open(host).expect("neutral reopen");
    let base = format!("https://{}", setup.local_addr());
    let tls_before = fs::read(setup.tls().ca_path()).expect("neutral CA");
    let admission_path = setup.admission_path();
    let retained = fs::read_to_string(&admission_path).expect("protected code retrieval");
    let code = retained.lines().nth(1).expect("host code").to_owned();
    let http = LocalTls::verification_client().expect("native HTTPS client");
    let mut cancellation = CancellationToken::new();
    let mut server = tokio::spawn(setup.serve_verified(http.clone(), cancellation.clone(), false));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if http
            .get(format!("{base}/idp/setup/status"))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "bounded startup");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let body = serde_json::json!({"operation_nonce": "0123456789abcdef0123456789abcdef", "administrator": "operator", "node": "API Node", "password": "fixture-initial-password"});
    let response = http
        .post(format!("{base}/idp/setup/create"))
        .json(&body)
        .send()
        .await
        .expect("unauthorized HTTPS Create");
    assert_eq!(
        response.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "real Create requires host code, not prior admission"
    );
    assert!(!root.join("idp").exists(), "denial cannot mutate owners");
    for authorization in [None, Some(format!("Setup {}", "0".repeat(64)))] {
        let mut request = http.post(format!("{base}/idp/setup/intent"));
        if let Some(authorization) = authorization {
            request = request.header("Authorization", authorization);
        }
        let response = request.send().await.expect("unauthorized intent lookup");
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
        assert!(response.bytes().await.expect("empty denial").is_empty());
    }
    let response = http
        .post(format!("{base}/idp/setup/intent"))
        .header("Authorization", format!("Setup {code}"))
        .send()
        .await
        .expect("authorized prechoice lookup");
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    assert!(
        response
            .bytes()
            .await
            .expect("empty unbound result")
            .is_empty()
    );
    let response = http
        .post(format!("{base}/idp/setup/intent"))
        .header("Authorization", format!("Setup {code}"))
        .header("Origin", "https://untrusted.example")
        .send()
        .await
        .expect("origin-protected intent lookup");
    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
    assert!(
        response
            .bytes()
            .await
            .expect("empty origin denial")
            .is_empty()
    );
    let first = http
        .post(format!("{base}/idp/setup/create"))
        .header("Authorization", format!("Setup {code}"))
        .json(&body)
        .send()
        .await
        .expect("admit representative interrupted Create");
    assert_eq!(first.status(), reqwest::StatusCode::ACCEPTED);
    let admitted_state =
        fs::read(&admission_path).expect("isolated durable pre-retirement checkpoint");
    let retained_intent = serde_json::json!({
        "operation_nonce": "0123456789abcdef0123456789abcdef",
        "administrator": "operator", "node": "API Node"
    });
    assert_eq!(
        http.post(format!("{base}/idp/setup/cancel"))
            .json(&retained_intent)
            .send()
            .await
            .expect("unauthorized cancellation")
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.post(format!("{base}/idp/setup/cancel"))
            .header("Authorization", format!("Setup {code}"))
            .json(&retained_intent)
            .send()
            .await
            .expect("cancel accepted owner work")
            .status(),
        reqwest::StatusCode::NO_CONTENT
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let status: serde_json::Value = http
            .get(format!("{base}/idp/setup/status"))
            .send()
            .await
            .expect("observe owner cancellation")
            .json()
            .await
            .expect("safe cancellation status");
        assert_eq!(status["ready"], false, "cancellation is never completion");
        if status["state"] == "unfinished" {
            break;
        }
        assert_eq!(status["state"], "creating");
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded owner cancellation"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    cancellation.cancel();
    server
        .await
        .expect("interrupted host task")
        .expect("interrupted host shutdown");
    let selected = BootstrapConfig::open_explicit(config.clone()).expect("exact incomplete host");
    let credentials =
        crate::CredentialBootstrap::open(selected.config()).expect("retained service credentials");
    let local =
        crate::LocalProvisioner::open(credentials.load().expect("protected retained credentials"))
            .expect("acknowledged incomplete owner progress");
    assert!(!local.is_complete().expect("incomplete owner readiness"));
    let retained_login_id = local
        .login_client()
        .expect("retained public login identity")
        .client_id
        .expect("reserved login Client ID");
    drop(local);
    drop(credentials);
    let reopened = SetupHost::open(selected).expect("incomplete host reopen");
    cancellation = CancellationToken::new();
    server = tokio::spawn(reopened.serve_verified(http.clone(), cancellation.clone(), false));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(response) = http
            .post(format!("{base}/idp/setup/intent"))
            .header("Authorization", format!("Setup {code}"))
            .send()
            .await
        {
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            assert_eq!(
                response
                    .json::<serde_json::Value>()
                    .await
                    .expect("recovered after restart"),
                retained_intent
            );
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded incomplete reopen"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let submitted = LocalTls::verification_client().expect("independent submitting client");
    let submitted_at = Instant::now();
    let response = submitted
        .post(format!("{base}/idp/setup/create"))
        .header("Authorization", format!("Setup {code}"))
        .json(&body)
        .send()
        .await
        .expect("actual authorized Create");
    assert_eq!(
        response.status(),
        reqwest::StatusCode::ACCEPTED,
        "Create must admit without waiting for owner initialization"
    );
    let accepted: serde_json::Value = response.json().await.expect("safe accepted result");
    assert_eq!(accepted["ready"], false);
    assert_eq!(accepted["state"], "creating");
    eprintln!(
        "Create accepted elapsed_ms={}",
        submitted_at.elapsed().as_millis()
    );
    drop(submitted);
    let response = http
        .post(format!("{base}/idp/setup/intent"))
        .header("Authorization", format!("Setup {code}"))
        .send()
        .await
        .expect("recover accepted intent after submitter disconnect");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let recovered: serde_json::Value = response.json().await.expect("retained nonsecret intent");
    assert_eq!(
        recovered,
        serde_json::json!({
            "operation_nonce": "0123456789abcdef0123456789abcdef",
            "administrator": "operator", "node": "API Node"
        })
    );
    assert!(!recovered.to_string().contains(&code));
    assert!(!recovered.to_string().contains("fixture-initial-password"));
    let mut conflict = body.clone();
    conflict["node"] = serde_json::json!("Changed Node");
    assert_eq!(
        http.post(format!("{base}/idp/setup/create"))
            .header("Authorization", format!("Setup {code}"))
            .json(&conflict)
            .send()
            .await
            .expect("active conflicting intent")
            .status(),
        reqwest::StatusCode::CONFLICT
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let result: serde_json::Value = loop {
        let response = http
            .get(format!("{base}/idp/setup/status"))
            .send()
            .await
            .expect("host-owned setup status");
        let result: serde_json::Value = response.json().await.expect("safe setup status");
        if result["ready"] == true {
            break result;
        }
        assert_eq!(
            result["state"], "creating",
            "accepted operation must finish without its submitting client"
        );
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded host-owned Create"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    eprintln!(
        "Create ready elapsed_ms={}",
        submitted_at.elapsed().as_millis()
    );
    let client_id = result["login_client"]["client_id"]
        .as_str()
        .expect("public login Client");
    assert_eq!(
        client_id, retained_login_id,
        "retry cannot duplicate the login Client"
    );
    assert!(!result.to_string().contains(&code));
    assert!(!result.to_string().contains("fixture-initial-password"));
    let session = http
        .post(format!("{base}/idp/oauth2/token"))
        .form(&[
            ("grant_type", "password"),
            ("client_id", client_id),
            ("username", "operator"),
            ("password", "fixture-initial-password"),
            ("scope", "openid"),
        ])
        .send()
        .await
        .expect("normal password grant");
    assert_eq!(session.status(), reqwest::StatusCode::OK);
    let token: model::contract::TokenResponse = session.json().await.expect("normal token");
    assert!(!token.access_token.0.is_empty());
    let retry = http
        .post(format!("{base}/idp/setup/create"))
        .json(&body)
        .send()
        .await
        .expect("completed normal-auth exact retry");
    assert_eq!(retry.status(), reqwest::StatusCode::OK);
    assert_eq!(
        http.post(format!("{base}/idp/setup/create"))
            .header("Authorization", format!("Setup {code}"))
            .json(&conflict)
            .send()
            .await
            .expect("changed payload")
            .status(),
        reqwest::StatusCode::CONFLICT
    );
    assert_eq!(
        http.post(format!("{base}/idp/setup/admission"))
            .header("Authorization", format!("Setup {code}"))
            .send()
            .await
            .expect("retired code")
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    let response = http
        .post(format!("{base}/idp/setup/intent"))
        .header("Authorization", format!("Setup {code}"))
        .send()
        .await
        .expect("retired intent lookup");
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert!(
        response
            .bytes()
            .await
            .expect("empty retired denial")
            .is_empty()
    );
    cancellation.cancel();
    server.await.expect("host task").expect("shutdown");
    fs::write(&admission_path, admitted_state)
        .expect("simulate stopped completion-before-retirement checkpoint");
    let host = BootstrapConfig::open_explicit(config.clone()).expect("completion-pending host");
    let pending = SetupHost::open(host).expect("reopen complete owners with unretired admission");
    let pending_shutdown = CancellationToken::new();
    let pending_server =
        tokio::spawn(pending.serve_verified(http.clone(), pending_shutdown.clone(), false));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(response) = http
            .post(format!("{base}/idp/setup/intent"))
            .header("Authorization", format!("Setup {code}"))
            .send()
            .await
        {
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            assert_eq!(
                response
                    .json::<serde_json::Value>()
                    .await
                    .expect("pending exact intent"),
                retained_intent
            );
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded pending completion startup"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        http.post(format!("{base}/idp/setup/create"))
            .header("Authorization", format!("Setup {code}"))
            .json(&body)
            .send()
            .await
            .expect("exact retirement continuation")
            .status(),
        reqwest::StatusCode::ACCEPTED,
        "completed owners with unretired admission must remain exactly recoverable"
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let status: serde_json::Value = http
            .get(format!("{base}/idp/setup/status"))
            .send()
            .await
            .expect("finalization status")
            .json()
            .await
            .expect("safe finalization result");
        if status["ready"] == true {
            assert_eq!(status["login_client"], result["login_client"]);
            break;
        }
        assert_eq!(status["state"], "creating");
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded exact finalization"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        http.post(format!("{base}/idp/setup/intent"))
            .header("Authorization", format!("Setup {code}"))
            .send()
            .await
            .expect("finalized capability denial")
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    pending_shutdown.cancel();
    pending_server
        .await
        .expect("finalization host task")
        .expect("finalization shutdown");
    let host = BootstrapConfig::open_explicit(config).expect("retained selected host");
    let reopened = SetupHost::open(host).expect("offline reopen, same neutral TLS");
    let cancellation = CancellationToken::new();
    let server = tokio::spawn(reopened.serve_verified(http.clone(), cancellation.clone(), false));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(response) = http.get(format!("{base}/idp/setup/status")).send().await {
            if let Ok(status) = response.json::<serde_json::Value>().await {
                if status["ready"] == true {
                    assert_eq!(status["login_client"], result["login_client"]);
                    break;
                }
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "completed offline reopen"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        http.post(format!("{base}/idp/setup/create"))
            .json(&body)
            .send()
            .await
            .expect("offline exact completed retry authenticates normally")
            .status(),
        reqwest::StatusCode::OK
    );
    cancellation.cancel();
    server
        .await
        .expect("reopened host task")
        .expect("offline shutdown");
    assert_eq!(
        fs::read(root.join("endpoint.tls/ca.pem")).expect("retained TLS"),
        tls_before
    );
}
