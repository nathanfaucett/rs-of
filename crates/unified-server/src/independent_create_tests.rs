use std::{
    fs,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use api::{IndependentCreateIntent, IndependentSetupHost, LocalTls};
use idp_model::model::Id;
use reqwest::{Client, StatusCode};
use tokio_util::sync::CancellationToken;

fn setup(root: &Path, owner: &str, address: SocketAddr) -> IndependentSetupHost {
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
    let root = std::env::temp_dir().join(format!("independent-create-{}", Id::now_v7()));
    fs::create_dir(&root).expect("isolated roots");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private roots");
    let mut addresses = Vec::new();
    let mut bundle = Vec::new();
    for owner in ["idp", "management", "storage"] {
        let port = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve listener");
        let address = port.local_addr().expect("address");
        let host = setup(&root, owner, address);
        bundle.extend(fs::read(host.tls().ca_path()).expect("owner CA"));
        addresses.push(address);
        drop(host);
        drop(port);
    }
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
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "independent_create_tests::independent_create_child",
            "--ignored",
            "--nocapture",
        ])
        .env("OF_INDEPENDENT_CREATE_ROOT", &root)
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

async fn start(
    root: &Path,
    addresses: &[SocketAddr],
    shutdown: &CancellationToken,
) -> Vec<tokio::task::JoinHandle<std::io::Result<()>>> {
    let mut idp = idp_server::AppConfig {
        data_dir: root.join("idp").to_string_lossy().into_owned(),
        ..Default::default()
    };
    idp.server.prefix = Some("/idp".into());
    let mut management = management_server::AppConfig {
        data_dir: root.join("management").to_string_lossy().into_owned(),
        ..Default::default()
    };
    management.server.prefix = Some("/management".into());
    let mut storage = storage_server::AppConfig {
        data_dir: root.join("storage").to_string_lossy().into_owned(),
        ..Default::default()
    };
    storage.server.prefix = Some("/storage".into());
    vec![
        tokio::spawn(idp_server::serve_independent_create(
            Arc::new(idp),
            setup(root, "idp", addresses[0]),
            shutdown.child_token(),
        )),
        tokio::spawn(management_server::serve_independent_create(
            Arc::new(management),
            setup(root, "management", addresses[1]),
            shutdown.child_token(),
        )),
        tokio::spawn(storage_server::serve_independent_create(
            Arc::new(storage),
            setup(root, "storage", addresses[2]),
            shutdown.child_token(),
        )),
    ]
}
async fn wait(http: &Client, intent: &IndependentCreateIntent, ready: bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    for owner in ["idp", "management", "storage"] {
        loop {
            if let Ok(response) = http
                .get(format!(
                    "{}/setup/status",
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
async fn normal_use(http: &Client, intent: &IndependentCreateIntent) -> String {
    let public: serde_json::Value = http
        .get(format!("{}/setup/independent/public", intent.idp))
        .send()
        .await
        .expect("public login discovery")
        .json()
        .await
        .expect("public identity");
    let client = public["facts"]["login_client"]
        .as_str()
        .expect("public login client");
    let response = http
        .post(format!("{}/oauth2/token", intent.idp))
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
    let response = http
        .get(format!("{}/devices/trusted", intent.management))
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
    public["facts"]["endpoint"]
        .as_str()
        .expect("actual Management endpoint")
        .into()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "parent supplies isolated native roots, mock trust command and mock secure provider"]
async fn independent_create_child() {
    let root = PathBuf::from(std::env::var_os("OF_INDEPENDENT_CREATE_ROOT").expect("parent roots"));
    idp_service::repo::PrivateKeyKeyringRepo::new_with_store(
        "independent-create",
        Arc::new(crate::local_provisioning_tests::SecretStore {
            store: keyring_core::mock::Store::new().expect("isolated provider"),
            fail_node_readback: Arc::new(AtomicBool::new(false)),
        }),
    );
    let addresses: Vec<SocketAddr> =
        serde_json::from_slice(&fs::read(root.join("addresses.json")).expect("addresses"))
            .expect("address JSON");
    let intent = IndependentCreateIntent {
        purpose: "create".into(),
        operation_nonce: "0123456789abcdef0123456789abcdef".into(),
        administrator: "operator".into(),
        node: "Independent Node".into(),
        idp: format!("https://{}/idp", addresses[0]),
        management: format!("https://{}/management", addresses[1]),
        storage: format!("https://{}/storage", addresses[2]),
        issuer: format!("https://{}/idp", addresses[0]),
        peer: None,
    };
    let http = LocalTls::verification_client().expect("native verified HTTPS");
    let shutdown = CancellationToken::new();
    let servers = start(&root, &addresses, &shutdown).await;
    wait(&http, &intent, false).await;
    let management_code = setup_code(&root, "management");
    let storage_code = setup_code(&root, "storage");
    let create = format!("{}/setup/create", intent.management);
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
    let retained = wait_public(&http, &intent.management).await;
    let recovered: serde_json::Value = http
        .post(format!("{}/setup/intent", intent.management))
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
    let forged = serde_json::json!({"operation":intent,"recipient":"management","registration":retained["clients"][0],"challenge":"a".repeat(64)});
    assert_eq!(
        http.post(format!("{}/setup/independent/challenge", intent.management))
            .json(&forged)
            .send()
            .await
            .expect("unsolicited challenge")
            .status(),
        StatusCode::FORBIDDEN
    );
    let cancel = format!("{}/setup/cancel", intent.management);
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
        http.post(format!("{}/setup/independent/challenge", intent.management))
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
    for owner in ["idp", "management", "storage"] {
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
            http.post(format!("{base}/setup/create"))
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
            .post(format!("{base}/setup/create"))
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
    let completed = wait_public(&http, &intent.management).await;
    assert_eq!(
        completed["clients"], retained["clients"],
        "interruption retains recipient-generated Client"
    );
    assert_eq!(
        completed["facts"]["endpoint"], retained["facts"]["endpoint"],
        "interruption retains the actual endpoint"
    );
    for owner in ["management", "storage"] {
        let base = intent.base(owner).expect("owner base");
        let code = if owner == "management" {
            &management_code
        } else {
            &storage_code
        };
        assert_eq!(
            http.post(format!("{base}/setup/intent"))
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
        http.get(format!("{}/devices/trusted", intent.management))
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
    let endpoint = normal_use(&http, &intent).await;
    assert_public_has_no_secrets(&root, &http, &intent).await;
    shutdown.cancel();
    for server in servers {
        server
            .await
            .expect("join owner")
            .expect("coordinated owner shutdown");
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
        .get(format!("{}/setup/status", intent.idp))
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
        .post(format!("{}/setup/intent", intent.idp))
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
        http.post(format!("{}/setup/create", intent.idp))
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
        normal_use(&http, &intent).await,
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
}

fn setup_code(root: &Path, owner: &str) -> String {
    fs::read_to_string(root.join(format!("{owner}.setup-host/setup-admission/state")))
        .expect("protected host code retrieval")
        .lines()
        .nth(1)
        .expect("host setup code")
        .to_owned()
}

async fn wait_public(http: &Client, base: &str) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(response) = http
            .get(format!("{base}/setup/independent/public"))
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
    for owner in ["idp", "management", "storage"] {
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
    for owner in ["idp", "management", "storage"] {
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
