#![cfg(unix)]

use std::{collections::HashMap, fs, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
use idp_model::model::Id;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use unified_server::{BootstrapConfig, BootstrapPurpose, SetupHost};

struct PausedProvider {
    store: Arc<keyring_core::mock::Store>,
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
}
impl keyring_core::api::CredentialStoreApi for PausedProvider {
    fn vendor(&self) -> String { self.store.vendor() }
    fn id(&self) -> String { self.store.id() }
    fn build(&self, service: &str, user: &str, _: Option<&HashMap<&str, &str>>) -> keyring_core::Result<keyring_core::Entry> {
        if user.contains(":node:") && !self.release.load(Ordering::SeqCst) {
            self.entered.store(true, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(30);
            while !self.release.load(Ordering::SeqCst) {
                if Instant::now() >= deadline { return Err(keyring_core::Error::NoEntry); }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        self.store.build(service, user, None)
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<keyring_core::Entry>> { self.store.search(spec) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

async fn observe(client: &reqwest::Client, base: &str, expected: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if let Ok(response) = client.get(format!("{base}/status")).send().await {
                let state: Value = response.json().await.expect("status JSON");
                if state["state"] == expected { break state; }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("bounded state transition")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn admitted_create_survives_concurrent_retries_cancellation_and_restart_without_reallocation() {
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    keyring_core::set_default_store(Arc::new(PausedProvider {
        store: keyring_core::mock::Store::new().expect("isolated provider"), entered: entered.clone(), release: release.clone(),
    }));
    let root = std::env::temp_dir().join(format!("host-sql-interrupted-{}", Id::now_v7()));
    let setup = SetupHost::create_http(BootstrapConfig::create(&root, BootstrapPurpose::AwaitingChoice, None).expect("fresh host")).expect("no OS trust");
    let code = SetupHost::protected_code(&root.join("setup-admission")).expect("protected read").expect("code");
    let address = setup.local_addr();
    let base = format!("http://{address}/idp/setup");
    let client = reqwest::Client::builder().timeout(Duration::from_secs(15)).build().expect("bounded client");
    let request = json!({"operation_nonce":"0123456789abcdef0123456789abcdef", "administrator":"operator", "node":"Retained Node", "password":"a sufficiently long password"});
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(setup.serve(shutdown.clone()));
    observe(&client, &base, "awaiting_choice").await;
    let auth = format!("Setup {code}");
    assert_eq!(client.post(format!("{base}/create")).header("Authorization", &auth).json(&request).send().await.expect("admitted operation").status(), 202);
    tokio::time::timeout(Duration::from_secs(20), async { while !entered.load(Ordering::SeqCst) { tokio::time::sleep(Duration::from_millis(10)).await; } }).await.expect("provider wait after durable attempt");
    assert_eq!(client.post(format!("{base}/intent")).send().await.expect("unauthorized recovery").status(), 401);
    let intent: Value = client.post(format!("{base}/intent")).header("Authorization", &auth).send().await.expect("authorized recovery").json().await.expect("nonsecret operation");
    assert_eq!(intent, json!({"operation_nonce":request["operation_nonce"], "administrator":"operator", "node":"Retained Node"}));
    let mut conflict = request.clone(); conflict["operation_nonce"] = json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let exact = client.post(format!("{base}/create")).header("Authorization", &auth).json(&request).send();
    let conflicting = client.post(format!("{base}/create")).header("Authorization", &auth).json(&conflict).send();
    let (exact, conflicting) = tokio::join!(exact, conflicting);
    assert_eq!(exact.expect("concurrent exact retry").status(), 202);
    assert_eq!(conflicting.expect("concurrent conflict").status(), 409);
    assert_eq!(client.post(format!("{base}/cancel")).header("Authorization", &auth).json(&intent).send().await.expect("authorized cancellation").status(), 204);
    release.store(true, Ordering::SeqCst);
    let stopped = observe(&client, &base, "unfinished").await;
    assert_eq!(stopped["ready"], false, "cancellation is not completion or rollback");
    shutdown.cancel();
    server.await.expect("joined host").expect("stopped worker");
    let host = BootstrapConfig::open(&root).expect("retained host");
    let references = host.local_reset_references().expect("retained allocations and attempted inventory");
    assert_eq!(references["keys"]["inventory_attempted"], true);
    let setup = SetupHost::open_http(host).expect("interrupted Create reopen");
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(setup.serve(shutdown.clone()));
    observe(&client, &base, "unfinished").await;
    let recovered: Value = client.post(format!("{base}/intent")).header("Authorization", &auth).send().await.expect("restarted intent recovery").json().await.expect("intent JSON");
    assert_eq!(recovered, intent);
    assert_eq!(client.post(format!("{base}/create")).header("Authorization", &auth).json(&conflict).send().await.expect("durable conflict").status(), 409);
    assert_eq!(client.post(format!("{base}/create")).header("Authorization", &auth).json(&request).send().await.expect("exact continuation").status(), 202);
    assert_eq!(observe(&client, &base, "ready").await["ready"], true);
    shutdown.cancel();
    server.await.expect("joined continuation").expect("stopped continuation");
    let host = BootstrapConfig::open(&root).expect("completed host");
    assert_eq!(host.local_reset_references().expect("same allocations"), references);
    host.delete_local_setup_secrets(&references).await.expect("exact isolated cleanup");
    drop(host);
    fs::remove_dir_all(root).expect("isolated cleanup");
}
