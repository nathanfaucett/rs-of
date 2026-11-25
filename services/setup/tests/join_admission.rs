#![cfg(all(unix, feature = "join"))]

use std::{
    fs,
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command},
    sync::Arc,
    time::Duration,
};

use axum::{
    body::{Body, to_bytes},
    http::{HeaderMap, HeaderValue, Request, header},
    response::Response,
};
use iroh_base::{EndpointId, SecretKey};
use reqwest::{Certificate, Client, StatusCode, redirect::Policy};
use serde_json::{Value, json};
use setup::{SetupAdmission, join::JoinHost};
use tempfile::tempdir;
use tokio::time::{sleep, timeout};
use tower::ServiceExt;

const ORIGIN: &str = "https://join-app.test";
const BIN: &str = env!("CARGO_BIN_EXE_join-node");

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn start(root: &Path, port: &str, url: &str, client: &Client) -> Server {
    let child = Server(
        Command::new(BIN)
            .arg("serve")
            .arg(root)
            .args([port, ORIGIN])
            .spawn()
            .expect("start HTTPS join helper"),
    );
    timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(response) = client.get(url).header("Origin", ORIGIN).send().await {
                assert_eq!(response.status(), StatusCode::OK);
                return;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("wait for verified HTTPS join helper");
    child
}

fn join_intent() -> Value {
    json!({
        "issuer": "https://installation.test/idp", "device": "https://installation.test/device",
        "storage": "https://installation.test/storage", "clientId": "join-fixture",
        "application": "https://installation.test/join-app",
        "acceptingPublicKey": SecretKey::generate().public().to_string(),
        "name": "Joining Node", "address": "127.0.0.1:23457"
    })
}

#[tokio::test]
#[allow(
    clippy::too_many_lines,
    reason = "one public HTTPS scenario covers admission and restart"
)]
async fn only_exact_https_setup_admission_can_prepare_and_resume_the_same_device() {
    let directory = tempdir().expect("isolated root");
    let root = directory.path().join("join");
    let port = TcpListener::bind("127.0.0.1:0")
        .expect("allocate loopback port")
        .local_addr()
        .expect("bound address")
        .port()
        .to_string();
    let url = format!("https://127.0.0.1:{port}/setup/join");
    let init = |origin: &str| {
        Command::new(BIN)
            .arg("init")
            .arg(&root)
            .args([&port, origin])
            .output()
            .expect("initialize join helper")
    };
    assert!(!init("http://join-app.test").status.success());
    assert!(!root.exists());
    assert!(init(ORIGIN).status.success());
    assert!(
        !init(ORIGIN).status.success(),
        "init must not replace a Node"
    );
    let code = SetupAdmission::protected_code(&root.join("admission"))
        .expect("read setup admission")
        .expect("fresh setup code");
    let ca = fs::read(root.join("tls/ca.pem")).expect("read helper CA");
    let client = Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(Policy::none())
        .add_root_certificate(Certificate::from_pem(&ca).expect("valid helper CA"))
        .build()
        .expect("verify TLS");
    let server = start(&root, &port, &url, &client).await;
    let request = || client.post(&url).header("Origin", ORIGIN);
    let authorized = || request().header("Authorization", format!("Setup {code}"));
    let mut intent = join_intent();
    let prepare = |intent: &Value| json!({ "action": "prepare", "intent": intent });
    for (builder, status) in [
        (request(), StatusCode::UNAUTHORIZED),
        (
            client
                .post(&url)
                .header("Origin", "https://other.test")
                .header("Authorization", format!("Setup {code}")),
            StatusCode::FORBIDDEN,
        ),
    ] {
        assert_eq!(
            builder
                .json(&prepare(&intent))
                .send()
                .await
                .expect("HTTP response")
                .status(),
            status
        );
    }
    intent["device"] = json!("http://installation.test/device");
    assert_eq!(
        authorized()
            .json(&prepare(&intent))
            .send()
            .await
            .expect("insecure Device base rejected")
            .status(),
        StatusCode::BAD_REQUEST
    );
    intent["device"] = json!("https://installation.test/device");
    let response = authorized()
        .json(&prepare(&intent))
        .send()
        .await
        .expect("prepare");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let view: Value = response.json().await.expect("public preparation evidence");
    let key = view["publicKey"].as_str().expect("endpoint public key");
    key.parse::<EndpointId>().expect("Iroh endpoint ID");
    assert_eq!(
        view,
        json!({"stage": "prepared", "intent": intent, "publicKey": key})
    );
    drop(server);
    let _restarted = start(&root, &port, &url, &client).await;
    let response = authorized()
        .json(&json!({"action": "resume"}))
        .send()
        .await
        .expect("resume");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .json::<Value>()
            .await
            .expect("retained public evidence"),
        view
    );
    intent["device"] = json!("https://different.test/device");
    assert_eq!(
        authorized()
            .json(&prepare(&intent))
            .send()
            .await
            .expect("changed target rejected")
            .status(),
        StatusCode::CONFLICT
    );
}

fn retain_approved_join(root: &Path, authorization: HeaderValue) {
    let path = root.join("join-v1.json");
    let mut journal: Value =
        serde_json::from_slice(&fs::read(&path).expect("read journal")).expect("parse journal");
    journal["phase"] = json!({"approved": "00000000-0000-0000-0000-000000000001"});
    fs::write(path, serde_json::to_vec(&journal).expect("encode journal")).expect("seed approval");
    let admission = SetupAdmission::open(&root.join("admission")).expect("reopen Join admission");
    let binding = admission
        .host_intent()
        .expect("read binding")
        .expect("bound Join");
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, authorization);
    admission.retire(&headers, &binding).expect("retire Join");
}

async fn proxy_post(
    host: &JoinHost,
    origin: &str,
    values: &[&HeaderValue],
    body: &Value,
) -> Response {
    let mut request = Request::post("/setup/join")
        .header(header::ORIGIN, origin)
        .header(header::CONTENT_TYPE, "application/json");
    for &value in values {
        request = request.header(header::AUTHORIZATION, value);
    }
    let request = request
        .body(Body::from(body.to_string()))
        .expect("POST request");
    host.router().oneshot(request).await.expect("POST response")
}

#[tokio::test]
async fn proxy_uses_node_code_but_retains_private_join_admission() {
    let directory = tempdir().expect("isolated root");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).expect("private root");
    let root = directory.path().join("join");
    let node_path = directory.path().join("node");
    let node = Arc::new(SetupAdmission::open(&node_path).expect("open Node admission"));
    let code = |path: &Path| SetupAdmission::protected_code(path).expect("read admission");
    let node_code = code(&node_path).expect("active Node code");
    let open = || JoinHost::open_for_proxy(&root, ORIGIN, Arc::clone(&node)).expect("open proxy");
    let host = open();
    let join_path = root.join("admission");
    let join_code = code(&join_path).expect("active private Join code");
    assert!(join_code != node_code, "admission codes must be separate");
    let auth = |code: &str| HeaderValue::try_from(format!("Setup {code}")).expect("setup header");
    let node_auth = auth(&node_code);
    let join_auth = auth(&join_code);
    let intent = join_intent();
    let prepare = json!({"action": "prepare", "intent": intent});
    let invalid = HeaderValue::from_static("Bearer invalid");
    let rejected: [&[&HeaderValue]; 4] =
        [&[], &[&join_auth], &[&invalid], &[&node_auth, &node_auth]];
    for values in rejected {
        let response = proxy_post(&host, ORIGIN, values, &prepare).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        proxy_post(&host, "https://other.test", &[&node_auth], &prepare)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert!(!root.join("join-v1.json").exists());
    let response = proxy_post(&host, ORIGIN, &[&node_auth], &prepare).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let prepared = to_bytes(response.into_body(), 8192)
        .await
        .expect("read prepared view");
    let view: Value = serde_json::from_slice(&prepared).expect("parse prepared view");
    let key = view["publicKey"].as_str().expect("endpoint public key");
    key.parse::<EndpointId>().expect("Iroh endpoint ID");
    assert_eq!(
        view,
        json!({"stage": "prepared", "intent": intent, "publicKey": key})
    );
    drop(host);

    let host = open();
    assert!(code(&join_path).is_some_and(|retained| retained == join_code));
    let resume = json!({"action": "resume"});
    let response = proxy_post(&host, ORIGIN, &[&node_auth], &resume).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 8192)
            .await
            .expect("read resumed view"),
        prepared
    );
    drop(host);

    retain_approved_join(&root, join_auth);
    let host = open();
    let response = proxy_post(&host, ORIGIN, &[&node_auth], &resume).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(code(&join_path).is_none());
    assert!(code(&node_path).is_some_and(|retained| retained == node_code));
}
