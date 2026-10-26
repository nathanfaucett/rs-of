#![cfg(unix)]

use std::{
    fs,
    process::Command,
    time::{Duration, Instant},
};

use idp_model::model::Id;
use tokio_util::sync::CancellationToken;

use unified_server::{BootstrapConfig, BootstrapPurpose, LocalTls, SetupHost};

#[tokio::test]
async fn fresh_host_publishes_complete_openapi_without_provisioning() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    let root = std::env::temp_dir().join(format!("unified-openapi-{}", Id::now_v7()));
    let host = BootstrapConfig::create_http(
        &root,
        BootstrapPurpose::AwaitingChoice,
        None,
        "127.0.0.1:3000".parse().expect("HTTP backend address"),
        "https://unified.localhost:1355".into(),
    )
    .expect("fresh neutral host");
    let config = host.config();
    let setup = SetupHost::create_http(host).expect("HTTP setup without certificates");
    let worker = setup.into_worker(CancellationToken::new());
    let router = worker.router().expect("neutral host router");
    for service in ["idp", "management", "storage"] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/{service}/openapi.json"))
                    .body(Body::empty())
                    .expect("OpenAPI request"),
            )
            .await
            .expect("OpenAPI response");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .expect("schema body");
        let document: serde_json::Value = serde_json::from_slice(&bytes).expect("OpenAPI JSON");
        let mut expected: serde_json::Value = serde_json::from_str(match service {
            "idp" => include_str!("../../../openapi/idp.json"),
            "management" => include_str!("../../../openapi/management.json"),
            "storage" => include_str!("../../../openapi/storage.json"),
            _ => unreachable!("known owner"),
        })
        .expect("canonical owner contract");
        expected["servers"] = document["servers"].clone();
        assert_eq!(
            document, expected,
            "served {service} contract must match export"
        );
        assert_eq!(
            document["servers"][0]["url"],
            format!("https://unified.localhost:1355/{service}")
        );
    }
    for service in ["idp", "management", "storage"] {
        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/{service}/version"))
                    .body(Body::empty())
                    .expect("version request"),
            )
            .await
            .expect("public version");
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }
    for (method, path, expected) in [
        ("GET", "/idp/.well-known/openid-configuration", 503),
        ("GET", "/idp/.well-known/jwks.json", 503),
        ("GET", "/idp/health", 503),
        ("GET", "/management/health", 503),
        ("GET", "/storage/health", 503),
        ("GET", "/storage", 503),
        ("GET", "/storage/", 404),
        ("POST", "/idp/oauth2/token", 503),
        ("GET", "/device/node-joins/acceptance", 503),
        ("PUT", "/device/node-joins/acceptance", 503),
        ("GET", "/device/node-joins/pending", 503),
        ("POST", "/device/node-joins/test/approve", 503),
        ("POST", "/device/node-joins/test/reject", 503),
        ("GET", "/storage/filesystems", 503),
        ("POST", "/storage/filesystems", 503),
        ("DELETE", "/storage/filesystems/test-id", 503),
        ("GET", "/storage/filesystems/test-id/residency", 503),
        ("POST", "/idp/.well-known/openid-configuration", 405),
        ("GET", "/idp/setup/openapi.json", 404),
        ("GET", "/idp/nonexistent", 404),
        ("GET", "/nonexistent", 404),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .expect("API request"),
            )
            .await
            .expect("stable API response");
        assert_eq!(response.status().as_u16(), expected, "{method} {path}");
        if expected == 503 {
            let bytes = to_bytes(response.into_body(), 1024)
                .await
                .expect("readiness response");
            let error: serde_json::Value = serde_json::from_slice(&bytes).expect("readiness JSON");
            assert_eq!(error["error"], "setup_required");
        }
    }
    let preflight = router
        .clone()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/storage/filesystems/test-id")
                .header("Origin", "https://unified.localhost:1355")
                .header("Access-Control-Request-Method", "DELETE")
                .header("Access-Control-Request-Headers", "authorization")
                .body(Body::empty())
                .expect("owner preflight"),
        )
        .await
        .expect("preflight response");
    assert_eq!(preflight.status(), axum::http::StatusCode::OK);
    assert!(
        preflight.headers()["access-control-allow-methods"]
            .to_str()
            .expect("allowed methods")
            .contains("DELETE")
    );
    for path in [
        &config.idp.data_dir,
        &config.device.data_dir,
        &config.storage.data_dir,
    ] {
        assert!(
            !std::path::Path::new(path).exists(),
            "schema export must not create owners"
        );
    }
    assert!(!config.endpoint_data_dir.exists());
    assert!(!config.credential_bootstrap_directory().exists());
    assert!(!config.local_provisioning_directory().exists());
    assert!(!LocalTls::directory(&config).exists());
    drop(worker);
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn http_setup_create_and_reopen_without_certificates() {
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", "http_setup_child", "--ignored", "--nocapture"])
        .env("RUST_MIN_STACK", "2097152")
        .spawn()
        .expect("isolated provider child");
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        if let Some(status) = child.try_wait().expect("child status") {
            assert!(status.success(), "HTTP setup child must succeed");
            break;
        }
        if Instant::now() >= deadline {
            child.kill().expect("bound child");
            child.wait().expect("reap child");
            panic!("HTTP setup child exceeded 90 seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "parent isolates the mock key provider"]
async fn http_setup_child() {
    keyring_core::set_default_store(std::sync::Arc::new(SecretStore(
        keyring_core::mock::Store::new().expect("isolated provider"),
    )));
    let root = std::env::temp_dir().join(format!("unified-http-{}", Id::now_v7()));
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").expect("test port");
    let address = reservation.local_addr().expect("test address");
    drop(reservation);
    let public = "https://unified.localhost";
    let host = BootstrapConfig::create_http(
        &root,
        BootstrapPurpose::AwaitingChoice,
        None,
        address,
        public.into(),
    )
    .expect("fresh proxy host");
    let config = host.config();
    assert_eq!(config.listen_addr, address);
    assert_eq!(config.idp.api_public_uri, format!("{public}/idp"));
    assert_eq!(config.storage.api_public_uri, format!("{public}/storage"));
    assert!(config.idp.oauth2.issuer.is_empty());
    let setup = SetupHost::create_http(host).expect("neutral HTTP without certificates");
    let admission_path = setup.admission_path();
    let state = fs::read_to_string(&admission_path).expect("protected host code");
    let code = state.lines().nth(1).expect("host code");
    let tls_directory = LocalTls::directory(&config);
    assert!(!tls_directory.exists());
    let http = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .expect("HTTP client");
    let base = format!("http://{address}");
    let client = of_idp_client::apis::configuration::Configuration {
        base_path: format!("{base}/idp"),
        client: http.clone(),
        ..Default::default()
    };
    let body = serde_json::json!({"operation_nonce":"0123456789abcdef0123456789abcdef", "administrator":"operator", "node":"HTTP Node", "password":"fixture-initial-password"});
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(setup.serve(shutdown.clone()));
    wait_status(&http, &base, false).await;
    for authorization in [None, Some(format!("Setup {}", "0".repeat(64)))] {
        let mut request = http.post(format!("{base}/idp/setup/create")).json(&body);
        if let Some(value) = authorization {
            request = request.header("Authorization", value);
        }
        assert_eq!(
            request.send().await.expect("unauthorized create").status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        http.post(format!("{base}/idp/setup/create"))
            .header("Authorization", format!("Setup {code}"))
            .header("Origin", "https://evil.invalid")
            .json(&body)
            .send()
            .await
            .expect("origin denial")
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    assert!(!std::path::Path::new(&config.idp.data_dir).exists());
    assert_eq!(
        http.post(format!("{base}/idp/setup/create"))
            .header("Authorization", format!("Setup {code}"))
            .header("Origin", public)
            .json(&body)
            .send()
            .await
            .expect("authorized create")
            .status(),
        reqwest::StatusCode::ACCEPTED
    );
    wait_status(&http, &base, true).await;
    assert!(!tls_directory.exists(), "API Create must not produce TLS");
    let completed = of_idp_client::apis::setup_api::setup_create(
        &client,
        of_idp_client::apis::setup_api::SetupCreateParams {
            create_request: of_idp_client::models::CreateRequest::new(
                "operator".into(),
                "HTTP Node".into(),
                "0123456789abcdef0123456789abcdef".into(),
                "fixture-initial-password".into(),
            ),
        },
    )
    .await
    .expect("authenticated completed retry over HTTP");
    assert!(completed.ready);
    // Generated operations do not expose the exact completed-retry HTTP status.
    assert_eq!(
        http.post(format!("{base}/idp/setup/create"))
            .json(&body)
            .send()
            .await
            .expect("exact completed retry status")
            .status(),
        reqwest::StatusCode::OK
    );
    let mut wrong = body.clone();
    wrong["password"] = serde_json::json!("wrong-password");
    assert_eq!(
        http.post(format!("{base}/idp/setup/create"))
            .json(&wrong)
            .send()
            .await
            .expect("completed retry password denial")
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    shutdown.cancel();
    server.await.expect("server task").expect("HTTP shutdown");

    let host = BootstrapConfig::open(&root).expect("retained proxy identity");
    assert_eq!(host.config().idp.api_public_uri, config.idp.api_public_uri);
    let setup = SetupHost::open_http(host).expect("reopen without certificates");
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(setup.serve(shutdown.clone()));
    wait_status(&http, &base, true).await;
    of_idp_client::apis::default_api::health(&client)
        .await
        .expect("normal API through generated owner client");
    shutdown.cancel();
    server
        .await
        .expect("reopened server task")
        .expect("reopened shutdown");
    assert!(!tls_directory.exists(), "reopen must not produce TLS");
    fs::remove_dir_all(root).expect("isolated cleanup");

    let root = std::env::temp_dir().join(format!("unified-http-cli-{}", Id::now_v7()));
    let host = BootstrapConfig::create_http(
        &root,
        BootstrapPurpose::Create,
        None,
        address,
        public.into(),
    )
    .expect("CLI create host");
    let config = host.config();
    let credentials = unified_server::CredentialBootstrap::create(config.clone())
        .expect("HTTP credentials without TLS");
    let mut local = unified_server::LocalProvisioner::create_with_password(
        credentials.load().expect("retained config"),
        "operator",
        "CLI Node",
        "fixture-initial-password",
    )
    .expect("CLI create intent");
    local
        .provision_http("fixture-initial-password")
        .await
        .expect("HTTP CLI provisioning");
    local.verify_http().await.expect("completed CLI retry");
    drop(local);
    let (runtime, local) = unified_server::LocalProvisioner::open_runtime(config.clone())
        .await
        .expect("CLI serve reopen");
    let shutdown = runtime.cancellation_token();
    let server = tokio::spawn(runtime.serve());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if of_idp_client::apis::default_api::health(&client)
            .await
            .is_ok()
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded CLI HTTP serving"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    shutdown.cancel();
    if let Err(error) = server.await.expect("CLI server task") {
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::Interrupted,
            "shutdown may interrupt background startup"
        );
    }
    assert!(
        !LocalTls::directory(&config).exists(),
        "CLI create/verify/serve must not produce TLS"
    );
    drop((local, credentials, host));
    fs::remove_dir_all(root).expect("CLI fixture cleanup");
}

async fn wait_status(http: &reqwest::Client, base: &str, ready: bool) {
    let client = of_idp_client::apis::configuration::Configuration {
        base_path: format!("{base}/idp"),
        client: http.clone(),
        ..Default::default()
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let last = match of_idp_client::apis::setup_api::setup_status(&client).await {
            Ok(status) => {
                if status.ready == ready {
                    return;
                }
                assert_ne!(status.state, "unavailable", "{status:?}");
                format!("{status:?}")
            }
            Err(error) => error.to_string(),
        };
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded HTTP setup readiness: {last}"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}

struct SecretStore(std::sync::Arc<keyring_core::mock::Store>);

impl keyring_core::api::CredentialStoreApi for SecretStore {
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
        // The mock has no platform target; IdP's target modifier is not a credential key.
        self.0.build(service, user, None)
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
