#![cfg(unix)]

use idp_model::model::Id;
use std::{collections::HashMap, fs, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
use unified_server::{BootstrapConfig, BootstrapPurpose, SetupHost};

struct SecretStore(Arc<keyring_core::mock::Store>);
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
        _: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<keyring_core::Entry> {
        self.0.build(service, user, None)
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<keyring_core::Entry>> {
        self.0.search(spec)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_retires_atomically_reopens_and_authenticates_completed_retries() {
    let store: Arc<keyring_core::CredentialStore> = Arc::new(SecretStore(
        keyring_core::mock::Store::new().expect("isolated provider"),
    ));
    keyring_core::set_default_store(store.clone());
    let root = std::env::temp_dir().join(format!("host-sql-create-{}", Id::now_v7()));
    let host =
        BootstrapConfig::create(&root, BootstrapPurpose::AwaitingChoice, None).expect("fresh host");
    let setup = SetupHost::create_http(host).expect("HTTP host without OS trust");
    let path = root.join("setup-admission");
    let code = SetupHost::protected_code(&path)
        .expect("protected retrieval")
        .expect("live code");
    let credential = fs::read_to_string(path.join("state")).expect("credential file");
    assert_eq!(credential, format!("setup-code-v1\n{code}"));
    let address = setup.local_addr();
    let shutdown = CancellationToken::new();
    let worker = setup.into_worker(shutdown.clone());
    let router = worker.router().expect("setup router");
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("isolated listener");
    let stopping = shutdown.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(stopping.cancelled_owned())
            .await
    });
    let owner = tokio::spawn(worker.run());
    let client = reqwest::Client::new();
    let base = format!("http://{address}/idp/setup");
    let request = serde_json::json!({"operation_nonce": "0123456789abcdef0123456789abcdef", "administrator": "operator", "node": "SQL Node", "password": "a sufficiently long password"});
    let response = client
        .post(format!("{base}/create"))
        .header("Authorization", format!("Setup {code}"))
        .json(&request)
        .send()
        .await
        .expect("Create response");
    assert_eq!(response.status(), 202);
    let result = tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            let response = client
                .get(format!("{base}/status"))
                .send()
                .await
                .expect("status");
            let status: serde_json::Value = response.json().await.expect("status JSON");
            if status["ready"] == true {
                break status;
            }
            assert_ne!(status["state"], "unfinished", "Create failed: {status}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("bounded Create completion");
    assert_eq!(
        SetupHost::protected_code(&path).expect("retired retrieval"),
        None
    );
    assert!(!path.join("state").exists());
    assert!(!root.join("host.json").exists());
    assert!(!root.join("endpoint.credentials/credentials.json").exists());
    assert!(!root.join("endpoint.provisioning/intent.json").exists());
    fs::write(path.join("state"), credential).expect("simulate failed code cleanup");
    assert_eq!(
        client
            .post(format!("{base}/admission"))
            .header("Authorization", format!("Setup {code}"))
            .send()
            .await
            .expect("leftover admission")
            .status(),
        401
    );
    assert_eq!(
        SetupHost::protected_code(&path).expect("leftover protected retrieval"),
        None
    );
    assert_eq!(
        client
            .post(format!("{base}/create"))
            .json(&request)
            .send()
            .await
            .expect("completed retry")
            .status(),
        200
    );
    let mut conflict = request.clone();
    conflict["operation_nonce"] = serde_json::json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert_eq!(
        client
            .post(format!("{base}/create"))
            .json(&conflict)
            .send()
            .await
            .expect("conflicting retry")
            .status(),
        409
    );
    let mut wrong = request.clone();
    wrong["password"] = serde_json::json!("not the administrator password");
    assert_eq!(
        client
            .post(format!("{base}/create"))
            .json(&wrong)
            .send()
            .await
            .expect("normal auth denial")
            .status(),
        401
    );
    shutdown.cancel();
    server
        .await
        .expect("server joined")
        .expect("server stopped");
    tokio::time::timeout(Duration::from_secs(15), owner)
        .await
        .expect("worker bounded shutdown")
        .expect("worker joined")
        .expect("worker stopped");
    let host = BootstrapConfig::open(&root).expect("retained SQL host");
    let references = host
        .local_reset_references()
        .expect("exact SQL reset references");
    assert!(!references["keys"].is_null());
    let setup = SetupHost::open_http(host).expect("retired host reopens without live authority");
    let shutdown = CancellationToken::new();
    let worker = setup.into_worker(shutdown.clone());
    let router = worker.router().expect("reopened router");
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("reopened listener");
    let stopping = shutdown.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(stopping.cancelled_owned())
            .await
    });
    let owner = tokio::spawn(worker.run());
    let reopened = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let status: serde_json::Value = client
                .get(format!("{base}/status"))
                .send()
                .await
                .expect("reopened status")
                .json()
                .await
                .expect("status JSON");
            if status["ready"] == true {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("bounded existing-only reopen");
    assert_eq!(reopened["login_client"], result["login_client"]);
    shutdown.cancel();
    server
        .await
        .expect("reopened server joined")
        .expect("reopened server stopped");
    tokio::time::timeout(Duration::from_secs(15), owner)
        .await
        .expect("bounded reopened shutdown")
        .expect("reopened worker joined")
        .expect("reopened worker stopped");
    let host = BootstrapConfig::open(&root).expect("stopped SQL host");
    host.delete_local_setup_secrets(&references)
        .await
        .expect("exact provider cleanup");
    drop(host);
    fs::remove_dir_all(root).expect("isolated cleanup");
}
