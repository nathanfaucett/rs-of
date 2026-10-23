use std::{
    fs,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

use api::{IndependentCreateIntent, IndependentSetupHost, LocalTls};
use idp_model::model::Id;
use reqwest::{Client, StatusCode};
use tokio_util::sync::CancellationToken;

#[path = "review_transfer_restart_tests.rs"]
mod review_transfer_restart_tests;

pub(super) fn setup(root: &Path, owner: &str, address: SocketAddr) -> IndependentSetupHost {
    IndependentSetupHost::prepare(
        owner,
        &root.join(owner),
        address,
        "https://localhost:5173",
        &format!("/{owner}"),
        false,
    )
    .expect("prepare independent owner")
    .expect("setup host")
}

#[test]
fn production_independent_create_and_offline_reopen() {
    run_production_create(false);
}

#[test]
fn production_independent_create_legacy_record_reset() {
    run_production_create(true);
}

fn run_production_create(legacy_reset: bool) {
    let root = std::env::temp_dir().join(format!("independent-create-{}", Id::now_v7()));
    fs::create_dir(&root).expect("isolated roots");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private roots");
    let mut addresses = Vec::new();
    let mut bundle = Vec::new();
    for owner in ["idp", "device", "storage"] {
        let port = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve listener");
        let address = port.local_addr().expect("address");
        let host = setup(&root, owner, address);
        bundle.extend(fs::read(host.tls().ca_path()).expect("owner CA"));
        addresses.push(address);
        drop(host);
        drop(port);
    }
    bundle.extend(crate::owner_composition_tests::reset::prepare_interrupted(
        &root,
        addresses[2],
    ));
    fs::write(
        root.join("addresses.json"),
        serde_json::to_vec(&addresses).expect("addresses JSON"),
    )
    .expect("addresses");
    fs::write(root.join("roots.pem"), bundle).expect("isolated native bundle");
    fs::create_dir(root.join("empty-roots")).expect("isolated native directory");
    fs::create_dir(root.join("commands")).expect("mock command directory");
    fs::write(root.join("commands/trust"), "#!/bin/sh\nexit 0\n").expect("isolated trust command");
    fs::set_permissions(
        root.join("commands/trust"),
        fs::Permissions::from_mode(0o700),
    )
    .expect("executable mock");
    crate::owner_composition_tests::reset::prepare_trust_removal(&root);
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "owner_composition_tests::create::independent_create_child",
            "--ignored",
            "--nocapture",
        ])
        .env("OF_INDEPENDENT_CREATE_ROOT", &root)
        .env(
            "OF_INDEPENDENT_LEGACY_RESET",
            if legacy_reset { "1" } else { "0" },
        )
        .env("SSL_CERT_FILE", root.join("roots.pem"))
        .env("SSL_CERT_DIR", root.join("empty-roots"))
        .env("PATH", root.join("commands"))
        .env("NO_PROXY", "127.0.0.1,::1,localhost")
        .stdin(Stdio::null())
        .spawn()
        .expect("isolated child");
    let deadline = Instant::now() + Duration::from_secs(90);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("bound child");
            child.wait().expect("reap child");
            panic!("independent Create exceeded 90 seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    fs::remove_dir_all(root).expect("isolated fixture cleanup");
    assert!(status.success(), "actual independent HTTPS Create child");
}

pub(super) async fn start(
    root: &Path,
    addresses: &[SocketAddr],
    shutdown: &CancellationToken,
) -> Vec<tokio::task::JoinHandle<std::io::Result<()>>> {
    let mut idp = idp_server::AppConfig {
        data_dir: root.join("idp").to_string_lossy().into_owned(),
        ..Default::default()
    };
    idp.server.prefix = Some("/idp".into());
    let mut device = device_server::AppConfig {
        data_dir: root.join("device").to_string_lossy().into_owned(),
        ..Default::default()
    };
    device.server.prefix = Some("/device".into());
    let mut storage = storage_server::AppConfig {
        data_dir: root.join("storage").to_string_lossy().into_owned(),
        ..Default::default()
    };
    storage.server.prefix = Some("/storage".into());
    if let Ok(bytes) = fs::read(root.join("shared-sql.json")) {
        let (identity, devices): (db::RemoteDatabaseConfig, db::RemoteDatabaseConfig) =
            serde_json::from_slice(&bytes).expect("remote setup endpoints");
        idp.identity_database = Some(identity.clone());
        device.identity_database = Some(identity);
        device.device_database = Some(devices);
    }
    vec![
        tokio::spawn({
            let task = idp_server::serve_independent_create(
                Arc::new(idp),
                setup(root, "idp", addresses[0]),
                shutdown.child_token(),
            );
            async move {
                task.await.inspect_err(|error| {
                    eprintln!("Independent fixture owner=idp stopped: {error}")
                })
            }
        }),
        tokio::spawn({
            let task = device_server::serve_independent_create(
                Arc::new(device),
                setup(root, "device", addresses[1]),
                shutdown.child_token(),
            );
            async move {
                task.await.inspect_err(|error| {
                    eprintln!("Independent fixture owner=device stopped: {error}")
                })
            }
        }),
        tokio::spawn({
            let task = storage_server::serve_independent_create(
                Arc::new(storage),
                setup(root, "storage", addresses[2]),
                shutdown.child_token(),
            );
            async move {
                task.await.inspect_err(|error| {
                    eprintln!("Independent fixture owner=storage stopped: {error}")
                })
            }
        }),
    ]
}
pub(super) async fn wait(http: &Client, intent: &IndependentCreateIntent, ready: bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    for owner in ["idp", "device", "storage"] {
        loop {
            if let Ok(response) = http
                .get(format!(
                    "{}/independent/setup/status",
                    intent.base(owner).expect("owner")
                ))
                .send()
                .await
            {
                if response.status() == StatusCode::OK {
                    let status: serde_json::Value = response.json().await.expect("status JSON");
                    if status["ready"] == ready {
                        break;
                    }
                }
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "bounded independent {owner} readiness"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
pub(super) async fn normal_use(
    http: &Client,
    intent: &IndependentCreateIntent,
) -> (String, String, String) {
    let public: serde_json::Value = http
        .get(format!(
            "{}/independent/setup/independent/public",
            intent.idp
        ))
        .send()
        .await
        .expect("public login discovery")
        .json()
        .await
        .expect("public identity");
    let client = public["facts"]["login_client"]
        .as_str()
        .expect("public login client");
    for origin in ["https://localhost:5173", "https://untrusted.example"] {
        let response = http
            .request(
                reqwest::Method::OPTIONS,
                format!("{}/oauth2/token", intent.idp),
            )
            .header("Origin", origin)
            .header("Access-Control-Request-Method", "POST")
            .header(
                "Access-Control-Request-Headers",
                "authorization,content-type",
            )
            .send()
            .await
            .expect("HTTPS Authorization preflight");
        assert_eq!(response.status(), StatusCode::OK);
        let allowed = response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok());
        assert_eq!(
            allowed == Some(origin),
            origin == "https://localhost:5173",
            "only configured browser origin may read normal routes"
        );
        if origin == "https://localhost:5173" {
            assert!(
                response.headers()["access-control-allow-headers"]
                    .to_str()
                    .expect("allowed headers")
                    .contains("authorization")
            );
        }
    }
    let response = http
        .post(format!("{}/oauth2/token", intent.idp))
        .header("Origin", "https://localhost:5173")
        .form(&[
            ("grant_type", "password"),
            ("client_id", client),
            ("username", "operator"),
            ("password", "protected-password"),
            ("scope", "openid"),
            ("resource", "idp-management"),
        ])
        .send()
        .await
        .expect("normal login");
    assert_eq!(response.status(), StatusCode::OK, "normal User login");
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        "https://localhost:5173",
        "HTTPS token response is browser-readable"
    );
    let hostile = http
        .post(format!("{}/oauth2/token", intent.idp))
        .header("Origin", "https://untrusted.example")
        .form(&[("grant_type", "password"), ("client_id", client)])
        .send()
        .await
        .expect("hostile browser token request");
    assert_ne!(
        hostile
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://untrusted.example")
    );
    let token: model::contract::TokenResponse = response.json().await.expect("normal token");
    let response = http
        .post(format!("{}/oauth2/token", intent.idp))
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            ("client_id", client),
            ("subject_token", token.access_token.0.as_str()),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("resource", "idp-management"),
        ])
        .send()
        .await
        .expect("Management audience exchange");
    assert_eq!(response.status(), StatusCode::OK);
    let token: model::contract::TokenResponse =
        response.json().await.expect("Management audience token");
    let management_token = token.access_token.0.clone();
    let response = http
        .get(format!("{}/devices/trusted", intent.device))
        .bearer_auth(&token.access_token.0)
        .send()
        .await
        .expect("normal Management use");
    assert_eq!(response.status(), StatusCode::OK);
    let devices: serde_json::Value = response.json().await.expect("trusted Devices");
    assert_eq!(
        devices.as_array().expect("Device list").len(),
        1,
        "topology creates one actual Management Device, not one per process"
    );
    let response = http
        .post(format!("{}/oauth2/token", intent.idp))
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            ("client_id", client),
            ("subject_token", token.access_token.0.as_str()),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("resource", "storage:databases"),
            (
                "authorization_details",
                r#"[{"type":"storage","actions":["read","write"]}]"#,
            ),
        ])
        .send()
        .await
        .expect("Storage token exchange");
    assert_eq!(response.status(), StatusCode::OK);
    let token: model::contract::TokenResponse = response.json().await.expect("Storage token");
    let response = http
        .get(format!("{}/databases", intent.storage))
        .bearer_auth(&token.access_token.0)
        .send()
        .await
        .expect("authenticated Storage");
    assert_eq!(response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response.json().await.expect("empty catalog");
    assert!(catalog.as_array().expect("catalog list").is_empty());
    (
        public["facts"]["endpoint"]
            .as_str()
            .expect("actual Management endpoint")
            .into(),
        management_token,
        token.access_token.0,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "parent supplies isolated native roots, mock trust command and mock secure provider"]
async fn independent_create_child() {
    let root = PathBuf::from(std::env::var_os("OF_INDEPENDENT_CREATE_ROOT").expect("parent roots"));
    crate::hosted_setup_integration_tests::capture_setup_timings();
    let reset_provider = crate::owner_composition_tests::reset::Provider::install();
    let fail_node_readback = reset_provider.fail_node_readback.clone();
    let addresses: Vec<SocketAddr> =
        serde_json::from_slice(&fs::read(root.join("addresses.json")).expect("addresses"))
            .expect("address JSON");
    let intent = IndependentCreateIntent {
        purpose: "create".into(),
        operation_nonce: "0123456789abcdef0123456789abcdef".into(),
        administrator: "operator".into(),
        node: "Independent Node".into(),
        idp: format!("https://{}/idp", addresses[0]),
        device: format!("https://{}/device", addresses[1]),
        storage: format!("https://{}/storage", addresses[2]),
        issuer: format!("https://{}/idp", addresses[0]),
        peer: None,
    };
    let http = LocalTls::verification_client().expect("native verified HTTPS");
    delayed_forged_https_callback(&root, &addresses, &intent, &http).await;
    let shutdown = CancellationToken::new();
    let servers = start(&root, &addresses, &shutdown).await;
    wait(&http, &intent, false).await;
    let management_code = setup_code(&root, "device");
    let storage_code = setup_code(&root, "storage");
    let create = format!("{}/independent/setup/create", intent.device);
    assert_eq!(
        http.post(&create)
            .header("Authorization", format!("Setup {storage_code}"))
            .json(&intent)
            .send()
            .await
            .expect("wrong owner code")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.post(&create)
            .header("Authorization", format!("Setup {management_code}"))
            .header("Origin", "https://untrusted.example")
            .json(&intent)
            .send()
            .await
            .expect("wrong origin")
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        http.post(&create)
            .header("Authorization", format!("Setup {management_code}"))
            .json(&intent)
            .send()
            .await
            .expect("accepted disconnected caller")
            .status(),
        StatusCode::ACCEPTED
    );
    let retained = wait_public(&http, &intent.device).await;
    let recovered: serde_json::Value = http
        .post(format!("{}/independent/setup/intent", intent.device))
        .header("Authorization", format!("Setup {management_code}"))
        .send()
        .await
        .expect("recover exact operation")
        .json()
        .await
        .expect("nonsecret operation");
    assert_eq!(
        recovered,
        serde_json::to_value(&intent).expect("exact intent")
    );
    assert_eq!(
        http.post(&create)
            .header("Authorization", format!("Setup {management_code}"))
            .json(&intent)
            .send()
            .await
            .expect("active exact retry")
            .status(),
        StatusCode::ACCEPTED
    );
    let mut conflicting = intent.clone();
    conflicting.node = "Competing Node".into();
    assert_eq!(
        http.post(&create)
            .header("Authorization", format!("Setup {management_code}"))
            .json(&conflicting)
            .send()
            .await
            .expect("competing intent")
            .status(),
        StatusCode::CONFLICT
    );
    let forged = serde_json::json!({"operation":intent,"recipient":"device","registration":retained["clients"][0],"challenge":"a".repeat(64)});
    assert_eq!(
        http.post(format!(
            "{}/independent/setup/independent/challenge",
            intent.device
        ))
        .json(&forged)
        .send()
        .await
        .expect("unsolicited challenge")
        .status(),
        StatusCode::FORBIDDEN
    );
    let cancel = format!("{}/independent/setup/cancel", intent.device);
    assert_eq!(
        http.post(&cancel)
            .json(&intent)
            .send()
            .await
            .expect("unauthorized cancellation")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.post(&cancel)
            .header("Authorization", format!("Setup {management_code}"))
            .json(&intent)
            .send()
            .await
            .expect("explicit cancellation")
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        http.post(format!(
            "{}/independent/setup/independent/challenge",
            intent.device
        ))
        .json(&forged)
        .send()
        .await
        .expect("cancelled challenge replay")
        .status(),
        StatusCode::FORBIDDEN
    );
    shutdown.cancel();
    for server in servers {
        server
            .await
            .expect("join interrupted owner")
            .expect("interrupted owner shutdown");
    }
    let worker_path = root.join("device.setup-host/worker.json");
    let worker = fs::read(&worker_path).expect("pre-domain worker evidence");
    let provider = idp_service::repo::RawKeyringRepo::new(format!(
        "independent-setup:{}",
        root.join("device.setup-host").display()
    ))
    .expect("isolated provider");
    let client = retained["clients"][0]["client_id"]
        .as_str()
        .expect("retained Client");
    let secret = provider
        .load("", "", client)
        .expect("provider read")
        .expect("consumer secret");
    let endpoint_key = provider
        .load("", "", "endpoint")
        .expect("endpoint read")
        .expect("endpoint key");
    assert!(
        !root.join("device").exists(),
        "fault precedes domain creation"
    );
    fs::remove_file(&worker_path).expect("inject lost initialized worker record");
    let prepared = setup(&root, "device", addresses[1]);
    assert!(
        prepared.into_create(CancellationToken::new()).is_err(),
        "lost pre-domain initialized record must refuse before exact retry"
    );
    assert!(!worker_path.exists(), "missing record is not regenerated");
    assert_eq!(
        provider.load("", "", client).expect("unchanged secret"),
        Some(secret)
    );
    assert_eq!(
        provider
            .load("", "", "endpoint")
            .expect("unchanged endpoint"),
        Some(endpoint_key)
    );
    fs::write(&worker_path, worker).expect("restore fault fixture evidence");
    fs::set_permissions(&worker_path, fs::Permissions::from_mode(0o600))
        .expect("restore private evidence permissions");
    let shutdown = CancellationToken::new();
    let servers = start(&root, &addresses, &shutdown).await;
    wait(&http, &intent, false).await;
    assert_eq!(
        http.post(&create)
            .header("Authorization", format!("Setup {management_code}"))
            .json(&conflicting)
            .send()
            .await
            .expect("retained conflicting intent")
            .status(),
        StatusCode::CONFLICT
    );
    let idp_code = setup_code(&root, "idp");
    let mut unretired_idp = None;
    for owner in ["idp", "device", "storage"] {
        let base = intent.base(owner).expect("base");
        assert_eq!(
            http.get(format!("{base}/devices/trusted"))
                .send()
                .await
                .expect("protected request")
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let mut request = serde_json::to_value(&intent).expect("intent JSON");
        if owner == "idp" {
            request["password"] = serde_json::json!("protected-password");
        }
        assert_eq!(
            http.post(format!("{base}/independent/setup/create"))
                .json(&request)
                .send()
                .await
                .expect("unauthorized")
                .status(),
            StatusCode::UNAUTHORIZED
        );
        let code = setup_code(&root, owner);
        let started = Instant::now();
        let response = http
            .post(format!("{base}/independent/setup/create"))
            .header("Authorization", format!("Setup {code}"))
            .json(&request)
            .send()
            .await
            .expect("accepted Create");
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        if owner == "idp" {
            unretired_idp = Some(
                fs::read(root.join("idp.setup-host/setup-admission/state"))
                    .expect("bound unretired admission fault snapshot"),
            );
        }
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "acceptance does not await provisioning"
        );
    }
    wait(&http, &intent, true).await;
    let finalized = fs::read(&worker_path).expect("finalized recipient evidence");
    assert_eq!(
        http.post(format!(
            "{}/independent/setup/independent/challenge",
            intent.device
        ))
        .json(&forged)
        .send()
        .await
        .expect("post-finalization callback")
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        fs::read(&worker_path).expect("unchanged finalized evidence"),
        finalized,
        "callback cannot progress after finalization"
    );
    let completed = wait_public(&http, &intent.device).await;
    assert_eq!(
        completed["clients"], retained["clients"],
        "interruption retains recipient-generated Client"
    );
    assert_eq!(
        completed["facts"]["endpoint"], retained["facts"]["endpoint"],
        "interruption retains the actual endpoint"
    );
    for owner in ["device", "storage"] {
        let base = intent.base(owner).expect("owner base");
        let code = if owner == "device" {
            &management_code
        } else {
            &storage_code
        };
        assert_eq!(
            http.post(format!("{base}/independent/setup/intent"))
                .header("Authorization", format!("Setup {code}"))
                .send()
                .await
                .expect("retired recovery")
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        http.get(format!("{}/databases", intent.storage))
            .send()
            .await
            .expect("unauthenticated Storage")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.get(format!("{}/devices/trusted", intent.device))
            .send()
            .await
            .expect("unauthenticated Management")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.get(format!("{}/databases", intent.storage))
            .bearer_auth("invalid-token")
            .send()
            .await
            .expect("invalid Storage token")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let (endpoint, management_token, storage_token) = normal_use(&http, &intent).await;
    assert_public_has_no_secrets(&root, &http, &intent).await;
    fail_node_readback.store(true, std::sync::atomic::Ordering::SeqCst);
    current_status(&http, &intent.idp, false).await;
    assert!(
        !fail_node_readback.load(std::sync::atomic::Ordering::SeqCst),
        "status checks the current protected signer, not an in-memory signer cache"
    );
    current_status(&http, &intent.idp, true).await;
    for (owner, slot) in [
        ("idp", "client"),
        ("device", "client"),
        ("device", "endpoint"),
        ("storage", "idp"),
        ("storage", "device"),
    ] {
        let path = root.join(format!("{owner}.setup-host/worker.json"));
        let before = fs::read(&path).expect("retained owner record");
        let record: serde_json::Value = serde_json::from_slice(&before).expect("record");
        let client = match slot {
            "endpoint" => Some("endpoint"),
            "idp" => record["clients"][0].as_str(),
            "device" => record["clients"][1].as_str(),
            _ => record["client"].as_str(),
        }
        .expect("exact protected owner slot");
        let provider = idp_service::repo::RawKeyringRepo::new(format!(
            "independent-setup:{}",
            root.join(format!("{owner}.setup-host")).display()
        ))
        .expect("isolated provider");
        let secret = provider
            .load("", "", client)
            .expect("consumer secret")
            .expect("present protected material");
        provider
            .delete("", "", client)
            .expect("inject current protected material loss");
        current_status(&http, intent.base(owner).expect("owner"), false).await;
        assert_eq!(
            http.get(format!("{}/databases", intent.storage))
                .send()
                .await
                .expect("normal route remains independently authenticated")
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            fs::read(&path).expect("unchanged owner evidence"),
            before,
            "observation must not repair or rewrite durable setup"
        );
        assert!(
            provider
                .load("", "", client)
                .expect("no regeneration")
                .is_none()
        );
        provider
            .store("", "", client, &secret)
            .expect("restore isolated fault injection only");
        current_status(&http, intent.base(owner).expect("owner"), true).await;
    }
    for child in ["databases", "filesystems"] {
        let catalog = root.join("storage").join(child);
        let backup = root.join(format!("fault-{child}"));
        fs::rename(&catalog, &backup).expect("inject lost local catalog root");
        current_status(&http, &intent.storage, false).await;
        assert!(
            !catalog.exists(),
            "status cannot reconstruct missing catalog roots"
        );
        fs::write(&catalog, b"corrupt catalog root").expect("inject malformed catalog root");
        current_status(&http, &intent.storage, false).await;
        assert_eq!(
            fs::read(&catalog).expect("malformed catalog preserved"),
            b"corrupt catalog root"
        );
        fs::remove_file(&catalog).expect("remove only isolated injected file");
        fs::rename(backup, catalog).expect("restore isolated catalog fault only");
    }
    current_status(&http, &intent.storage, true).await;
    shutdown.cancel();
    for server in servers {
        server
            .await
            .expect("join owner")
            .expect("coordinated owner shutdown");
    }
    for child in ["databases", "filesystems"] {
        let catalog = root.join("storage").join(child);
        let backup = root.join(format!("stopped-fault-{child}"));
        fs::rename(&catalog, &backup).expect("stopped original catalog");
        fs::write(&catalog, b"corrupt stopped catalog root").expect("isolated malformed catalog");
        let record =
            fs::read(root.join("storage.setup-host/worker.json")).expect("exact stopped worker");
        let mut config = storage_server::AppConfig {
            data_dir: root.join("storage").to_string_lossy().into_owned(),
            ..Default::default()
        };
        config.server.prefix = Some("/storage".into());
        assert!(
            storage_server::serve_independent_create(
                Arc::new(config),
                setup(&root, "storage", addresses[2]),
                CancellationToken::new()
            )
            .await
            .is_err(),
            "completed reopen must refuse malformed catalog roots"
        );
        assert_eq!(
            fs::read(&catalog).expect("malformed catalog was not replaced"),
            b"corrupt stopped catalog root"
        );
        assert_eq!(
            fs::read(root.join("storage.setup-host/worker.json")).expect("worker intact"),
            record
        );
        fs::remove_file(&catalog).expect("remove only isolated injected file");
        fs::rename(backup, catalog).expect("restore isolated stopped fault");
    }
    fs::write(
        root.join("idp.setup-host/setup-admission/state"),
        unretired_idp.expect("bound admission snapshot"),
    )
    .expect("inject completed-before-retirement interruption");
    let shutdown = CancellationToken::new();
    let servers = start(&root, &addresses, &shutdown).await;
    wait_public(&http, &intent.idp).await;
    let status: serde_json::Value = http
        .get(format!("{}/independent/setup/status", intent.idp))
        .send()
        .await
        .expect("unfinished completion")
        .json()
        .await
        .expect("status");
    assert_eq!(
        status["ready"], false,
        "unretired completion never automatically becomes ready"
    );
    let recovered: serde_json::Value = http
        .post(format!("{}/independent/setup/intent", intent.idp))
        .header("Authorization", format!("Setup {idp_code}"))
        .send()
        .await
        .expect("recover exact finalization")
        .json()
        .await
        .expect("retained operation");
    assert_eq!(
        recovered,
        serde_json::to_value(&intent).expect("exact intent")
    );
    let mut finalize = serde_json::to_value(&intent).expect("finalize intent");
    finalize["password"] = serde_json::json!("protected-password");
    assert_eq!(
        http.post(format!("{}/independent/setup/create", intent.idp))
            .header("Authorization", format!("Setup {idp_code}"))
            .json(&finalize)
            .send()
            .await
            .expect("authorized exact finalization")
            .status(),
        StatusCode::ACCEPTED
    );
    wait(&http, &intent, true).await;
    assert_eq!(
        normal_use(&http, &intent).await.0,
        endpoint,
        "offline existing-only reopen retains endpoint"
    );
    shutdown.cancel();
    for server in servers {
        server
            .await
            .expect("join owner reopen")
            .expect("offline owner shutdown");
    }
    let endpoints = seed_endpoint_negatives(&root).await;
    review_transfer_restart_tests::qualify(&root, &addresses, &intent, &http, &management_token)
        .await;
    let shutdown = CancellationToken::new();
    let servers = start(&root, &addresses, &shutdown).await;
    wait(&http, &intent, true).await;
    exercise_endpoint_negatives(
        &root,
        &http,
        &intent,
        &endpoints,
        &management_token,
        &storage_token,
    )
    .await;
    shutdown.cancel();
    for server in servers {
        server
            .await
            .expect("endpoint regression listener")
            .expect("endpoint regression stopped reopen");
    }
    let shutdown = CancellationToken::new();
    let servers = start(&root, &addresses, &shutdown).await;
    wait(&http, &intent, true).await;
    let service = storage_policy_token(&root, &http, &intent).await;
    assert_eq!(
        http.get(format!(
            "{}/replication/devices/{}/selections",
            intent.device, endpoints[0].0
        ))
        .bearer_auth(&service)
        .send()
        .await
        .expect("retained learned revocation on a new connection after offline reopen")
        .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    current_status(&http, &intent.device, true).await;
    revoke_current_signer(&root, &http, &intent, &management_token).await;
    current_status(&http, &intent.idp, false).await;
    assert_eq!(
        http.get(format!("{}/databases", intent.storage))
            .send()
            .await
            .expect("normal authentication after signer readiness failure")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    shutdown.cancel();
    for server in servers {
        server
            .await
            .expect("learned signer listener")
            .expect("learned signer shutdown");
    }
    let before =
        fs::read(root.join("storage.setup-host/worker.json")).expect("retained Storage record");
    let record: serde_json::Value = serde_json::from_slice(&before).expect("record JSON");
    let client = record["clients"][0].as_str().expect("retained Client");
    let provider = idp_service::repo::RawKeyringRepo::new(format!(
        "independent-setup:{}",
        root.join("storage.setup-host").display()
    ))
    .expect("isolated provider");
    provider
        .delete("", "", client)
        .expect("inject protected relationship loss");
    let mut config = storage_server::AppConfig {
        data_dir: root.join("storage").to_string_lossy().into_owned(),
        ..Default::default()
    };
    config.server.prefix = Some("/storage".into());
    assert!(
        storage_server::serve_independent_create(
            Arc::new(config),
            setup(&root, "storage", addresses[2]),
            CancellationToken::new()
        )
        .await
        .is_err(),
        "missing attempted material refuses reopen without replacement"
    );
    assert_eq!(
        fs::read(root.join("storage.setup-host/worker.json")).expect("unchanged record"),
        before
    );
    assert!(
        provider
            .load("", "", client)
            .expect("provider remains missing")
            .is_none()
    );
    crate::owner_composition_tests::reset::qualify_completed(
        &root,
        &reset_provider,
        &intent.issuer,
    )
    .await;
    crate::owner_composition_tests::reset::qualify_interrupted(
        &root,
        addresses[2],
        &reset_provider,
    )
    .await;
}

pub(super) async fn seed_endpoint_negatives(root: &Path) -> Vec<(String, StatusCode, bool)> {
    use device_service::DeviceRepo;
    use device_service::replica::DbDeviceRepo;
    use device_service::replica::DbSelectionPolicyRepo;
    use device_service::replica::SelectionPolicy;
    use idp_model::contract::IdentityAction;
    use management_service::ManagementService;
    use management_service::replica::DbPermissionRepo;
    use management_service::replica::DbRoleRepo;
    let record: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("device.setup-host/worker.json")).expect("stopped Management evidence"),
    )
    .expect("record");
    let owner: Id = serde_json::from_value(record["user"].clone()).expect("setup owner");
    let role: Id = serde_json::from_value(record["create"]["role"].clone())
        .expect("Create administrator role");
    let engine = Arc::new(
        db::Database::open(root.join("device/devices.redb")).expect("existing-only stopped owner"),
    );
    let service = ManagementService::new(
        DbPermissionRepo::new(engine.client()),
        DbRoleRepo::new(engine.client()),
    );
    for action in [
        IdentityAction::DevicesRevoke,
        IdentityAction::DeviceRestrictionsUpdate,
    ] {
        let permission = service
            .create_permission(Id::nil(), action.permission(), None)
            .await
            .expect("explicit fixture permission");
        service
            .add_permission_to_role(Id::nil(), role, permission.id)
            .await
            .expect("ordinary fixture administrator grant");
    }
    let devices = DbDeviceRepo::new(engine.clone().client());
    let policies = DbSelectionPolicyRepo::new(engine.clone().client());
    let mut cases = Vec::new();
    for case in [
        "approved-no-signer",
        "pending",
        "revoked",
        "mismatched-policy-owner",
        "unknown",
    ] {
        let endpoint = iroh::SecretKey::generate().public().to_string();
        let device = Id::now_v7();
        if case == "pending" {
            devices
                .create(
                    owner.to_string(),
                    case.into(),
                    endpoint.clone(),
                    String::new(),
                    vec![1],
                    4_102_444_800,
                )
                .await
                .expect("pending Device is not approval");
        } else if case != "unknown" {
            devices
                .enroll_approved_exact(device, owner, case, &endpoint, 1)
                .await
                .expect("Management-only approved endpoint; no IdP signer is enrolled");
            policies
                .set_prevalidated(SelectionPolicy {
                    device_id: device,
                    owner_subject: if case == "mismatched-policy-owner" {
                        Id::now_v7().to_string()
                    } else {
                        owner.to_string()
                    },
                    application_id: None,
                    selected_kind: None,
                    selected_id: None,
                    admin_allowed: true,
                })
                .await
                .expect("exact current Device policy");
            if case == "revoked" {
                devices
                    .revoke_self(&endpoint)
                    .await
                    .expect("persisted endpoint revocation");
            }
        }
        cases.push((
            endpoint,
            if matches!(case, "approved-no-signer" | "mismatched-policy-owner") {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            },
            case == "mismatched-policy-owner",
        ));
    }
    cases
}

async fn storage_policy_token(
    root: &Path,
    http: &Client,
    intent: &IndependentCreateIntent,
) -> String {
    let record: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("storage.setup-host/worker.json")).expect("consumer record"),
    )
    .expect("record");
    let client = record["clients"][1]
        .as_str()
        .expect("Storage to Management Client");
    let secret = idp_service::setup_credentials::load_setup_secret(
        &format!(
            "independent-setup:{}",
            root.join("storage.setup-host").display()
        ),
        client,
    )
    .expect("protected consumer secret");
    let response = http
        .post(format!("{}/oauth2/token", intent.idp))
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", client),
            ("client_secret", secret.as_str()),
            (
                "scope",
                "management.device.list device.replication.read device.replication.admit",
            ),
            ("audience", intent.storage.as_str()),
        ])
        .send()
        .await
        .expect("real scoped service OAuth token");
    if response.status() != StatusCode::OK {
        panic!(
            "scoped service OAuth refused: {} {}",
            response.status(),
            response.text().await.expect("OAuth error response")
        );
    }
    response
        .json::<model::contract::TokenResponse>()
        .await
        .expect("service token")
        .access_token
        .0
}

async fn exercise_endpoint_negatives(
    root: &Path,
    http: &Client,
    intent: &IndependentCreateIntent,
    endpoints: &[(String, StatusCode, bool)],
    device: &str,
    storage: &str,
) {
    let token = storage_policy_token(root, http, intent).await;
    let mut approved = None;
    for (endpoint, expected, cleanup) in endpoints {
        let response = http
            .get(format!(
                "{}/replication/devices/{endpoint}/selections",
                intent.device
            ))
            .bearer_auth(&token)
            .send()
            .await
            .expect("same valid OAuth token is not endpoint approval");
        assert_eq!(response.status(), *expected);
        if *expected == StatusCode::OK {
            let evidence: serde_json::Value = response
                .json()
                .await
                .expect("current signer-free Management evidence");
            assert_eq!(evidence["endpointId"], *endpoint);
            if *cleanup {
                assert_eq!(evidence["adminAllowed"], false);
                assert_eq!(evidence["resources"], serde_json::json!([]));
                let pending = &evidence["pendingCleanup"];
                assert_eq!(pending["endpointId"], *endpoint);
                assert_eq!(pending["deviceId"], evidence["deviceId"]);
                assert_ne!(pending["oldOwnerSubject"], pending["newOwnerSubject"]);
                assert!(
                    pending["cleanupId"]
                        .as_str()
                        .expect("exact cleanup obligation")
                        .parse::<Id>()
                        .is_ok_and(|id| !id.is_nil())
                );
                continue;
            }
            assert!(evidence["pendingCleanup"].is_null());
            assert_eq!(evidence["adminAllowed"], true);
            approved = Some(
                evidence["deviceId"]
                    .as_str()
                    .expect("approved Device")
                    .to_owned(),
            );
        }
    }
    assert_eq!(
        http.delete(format!(
            "{}/devices/{}",
            intent.device,
            approved.expect("signer-free Device")
        ))
        .bearer_auth(device)
        .send()
        .await
        .expect("ordinary User learns exact endpoint revocation")
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        http.get(format!(
            "{}/replication/devices/{}/selections",
            intent.device, endpoints[0].0
        ))
        .bearer_auth(&token)
        .send()
        .await
        .expect("learned denial on the existing client/connection and still-valid service token")
        .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let public = wait_public(http, &intent.device).await;
    let device = public["facts"]["device"]
        .as_str()
        .expect("local Management Device");
    for allowed in [false, true] {
        assert_eq!(
            http.put(format!("{}/devices/{device}/restriction", intent.device))
                .bearer_auth(device)
                .json(&serde_json::json!({"adminAllowed":allowed}))
                .send()
                .await
                .expect("ordinary exact policy update")
                .status(),
            StatusCode::NO_CONTENT
        );
        current_status(http, &intent.device, allowed).await;
        current_status(http, &intent.storage, allowed).await;
        let normal = http
            .get(format!("{}/databases", intent.storage))
            .bearer_auth(storage)
            .send()
            .await
            .expect("normal protected operation independently rechecks current endpoint policy");
        assert_eq!(
            normal.status(),
            if allowed {
                StatusCode::OK
            } else {
                StatusCode::UNAUTHORIZED
            }
        );
    }
}

async fn revoke_current_signer(
    root: &Path,
    http: &Client,
    intent: &IndependentCreateIntent,
    device: &str,
) {
    let public = wait_public(http, &intent.idp).await;
    let response = http
        .post(format!("{}/oauth2/token", intent.idp))
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            (
                "client_id",
                public["facts"]["login_client"]
                    .as_str()
                    .expect("public login"),
            ),
            ("subject_token", device),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("resource", intent.idp.as_str()),
        ])
        .send()
        .await
        .expect("ordinary IdP owner audience exchange");
    assert_eq!(response.status(), StatusCode::OK);
    let token: model::contract::TokenResponse = response.json().await.expect("IdP User token");
    let record: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("idp.setup-host/worker.json")).expect("retained exact signer identity"),
    )
    .expect("record");
    assert_eq!(
        http.delete(format!(
            "{}/replica-signers/{}",
            intent.idp,
            record["member"].as_str().expect("local signer member")
        ))
        .bearer_auth(token.access_token.0)
        .send()
        .await
        .expect("ordinary authorized signer revocation")
        .status(),
        StatusCode::OK
    );
}

pub(super) async fn current_status(http: &Client, base: &str, ready: bool) {
    let response = http
        .get(format!("{base}/independent/setup/status"))
        .send()
        .await
        .expect("current owner setup observation");
    assert_eq!(
        response.status(),
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        }
    );
    let status: serde_json::Value = response.json().await.expect("nonsecret current status");
    assert_eq!(status["ready"], ready);
    assert_eq!(status["state"], if ready { "ready" } else { "unavailable" });
}

async fn delayed_forged_https_callback(
    root: &Path,
    addresses: &[SocketAddr],
    intent: &IndependentCreateIntent,
    http: &Client,
) {
    let idp = setup(root, "idp", addresses[0]);
    let tls = idp.tls().clone();
    drop(idp);
    let listener = tokio::net::TcpListener::bind(addresses[0])
        .await
        .expect("delayed IdP listener");
    let reached = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let observed = reached.clone();
    let blocked = release.clone();
    let router = axum::Router::new().route(
        "/idp/independent/setup/independent/register",
        axum::routing::post(move || {
            let observed = observed.clone();
            let blocked = blocked.clone();
            async move {
                observed.notify_one();
                blocked.notified().await;
                StatusCode::NO_CONTENT
            }
        }),
    );
    let stop = CancellationToken::new();
    let stopped = stop.clone();
    let remote = tokio::spawn(async move {
        axum::serve(tls.listener(listener), router)
            .with_graceful_shutdown(stopped.cancelled_owned())
            .await
    });
    let (host, mut commands) = setup(root, "device", addresses[1])
        .into_create(stop.child_token())
        .expect("recipient host");
    let control = host.control();
    let recipient = tokio::spawn(host.serve());
    let code = setup_code(root, "device");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if http
            .get(format!("{}/independent/setup/status", intent.device))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        assert!(Instant::now() < deadline, "bounded recipient listener");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        http.post(format!("{}/independent/setup/create", intent.device))
            .header("Authorization", format!("Setup {code}"))
            .json(intent)
            .send()
            .await
            .expect("admitted recipient")
            .status(),
        StatusCode::ACCEPTED
    );
    let command = commands.recv().await.expect("host-owned operation");
    let registration = serde_json::json!({"client_id":"00000000-0000-4000-8000-000000000001"});
    control
        .set_public(
            api::IndependentOwnerPublic {
                operation: intent.clone(),
                owner: "device".into(),
                clients: vec![registration.clone()],
                facts: serde_json::json!({}),
                registered: vec![],
            },
            Arc::new(|_| Ok("b".repeat(64))),
        )
        .await
        .expect("arranged public recipient");
    let forged = serde_json::json!({"operation":intent,"recipient":"device","registration":registration,"challenge":"a".repeat(64)});
    let request = http
        .post(format!(
            "{}/independent/setup/independent/challenge",
            intent.device
        ))
        .json(&forged);
    let callback = tokio::spawn(async move { request.send().await });
    tokio::time::timeout(Duration::from_secs(5), reached.notified())
        .await
        .expect("forged callback reached delayed verified HTTPS IdP");
    let cancellation = tokio::time::timeout(
        Duration::from_millis(250),
        http.post(format!("{}/independent/setup/cancel", intent.device))
            .header("Authorization", format!("Setup {code}"))
            .json(intent)
            .send(),
    )
    .await;
    release.notify_one();
    assert_eq!(
        cancellation
            .expect("outbound callback does not hold lifecycle gate")
            .expect("HTTPS cancellation")
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        callback
            .await
            .expect("callback task")
            .expect("callback response")
            .status(),
        StatusCode::FORBIDDEN
    );
    assert!(
        !control
            .registration_confirmed(intent, &registration)
            .expect("no post-cancel receipt")
    );
    assert!(
        !control
            .registered(registration["client_id"].as_str().expect("Client"))
            .await
    );
    assert!(
        control
            .finish(&command, axum::Router::new(), &serde_json::json!({}))
            .await
            .is_err(),
        "cancelled operation cannot finalize"
    );
    assert_eq!(
        http.get(format!("{}/independent/setup/status", intent.device))
            .send()
            .await
            .expect("responsive status")
            .json::<serde_json::Value>()
            .await
            .expect("status")["ready"],
        false
    );
    stop.cancel();
    recipient
        .await
        .expect("recipient task")
        .expect("recipient shutdown");
    remote
        .await
        .expect("delayed IdP task")
        .expect("delayed IdP shutdown");
}

pub(super) fn setup_code(root: &Path, owner: &str) -> String {
    fs::read_to_string(root.join(format!("{owner}.setup-host/setup-admission/state")))
        .expect("protected host code retrieval")
        .lines()
        .nth(1)
        .expect("host setup code")
        .to_owned()
}

pub(super) async fn wait_public(http: &Client, base: &str) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(response) = http
            .get(format!("{base}/independent/setup/independent/public"))
            .send()
            .await
        {
            if response.status() == StatusCode::OK {
                return response.json().await.expect("public owner evidence");
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded owner discovery"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn assert_public_has_no_secrets(
    root: &Path,
    http: &Client,
    intent: &IndependentCreateIntent,
) {
    let mut secrets = vec!["protected-password".to_owned()];
    for owner in ["idp", "device", "storage"] {
        let public = wait_public(http, intent.base(owner).expect("owner")).await;
        let provider = idp_service::repo::RawKeyringRepo::new(format!(
            "independent-setup:{}",
            root.join(format!("{owner}.setup-host")).display()
        ))
        .expect("isolated provider");
        for client in public["clients"].as_array().expect("public registrations") {
            let slot = client["client_id"].as_str().expect("public Client ID");
            secrets.push(
                String::from_utf8(
                    provider
                        .load("", "", slot)
                        .expect("provider read")
                        .expect("retained secret"),
                )
                .expect("secret text"),
            );
        }
    }
    for owner in ["idp", "device", "storage"] {
        let base = intent.base(owner).expect("owner");
        for route in ["setup/status", "setup/independent/public"] {
            let body = http
                .get(format!("{base}/{route}"))
                .send()
                .await
                .expect("public request")
                .text()
                .await
                .expect("public body");
            assert!(
                secrets.iter().all(|secret| !body.contains(secret)),
                "public contracts contain no domain secret"
            );
        }
        let body = fs::read_to_string(root.join(format!("{owner}.setup-host/worker.json")))
            .expect("durable nonsecret evidence");
        assert!(
            secrets.iter().all(|secret| !body.contains(secret)),
            "durable worker evidence contains no domain secret"
        );
    }
}
