#![cfg(all(unix, feature = "join"))]

use std::{
    fs,
    net::TcpListener,
    path::Path,
    process::{Child, Command},
    time::Duration,
};

use iroh_base::{EndpointId, SecretKey};
use reqwest::{Certificate, Client, StatusCode, redirect::Policy};
use serde_json::{Value, json};
use setup::SetupAdmission;
use tempfile::tempdir;
use tokio::time::{sleep, timeout};

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
    let mut intent = json!({
        "issuer": "https://installation.test/idp", "device": "https://installation.test/device",
        "storage": "https://installation.test/storage", "clientId": "join-fixture",
        "application": "https://installation.test/join-app",
        "acceptingPublicKey": SecretKey::generate().public().to_string(),
        "name": "Joining Node", "address": "127.0.0.1:23457"
    });
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
