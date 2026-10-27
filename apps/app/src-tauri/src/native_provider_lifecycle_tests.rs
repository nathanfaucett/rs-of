use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use serde_json::json;
use unified_server::LocalTls;

use super::NativeHost;

struct IsolatedProvider {
    store: Arc<keyring_core::mock::Store>,
    unavailable: Arc<AtomicBool>,
}

impl keyring_core::api::CredentialStoreApi for IsolatedProvider {
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
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(keyring_core::Error::NoEntry);
        }
        keyring_core::api::CredentialStoreApi::build(&*self.store, service, user, None)
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<keyring_core::Entry>> {
        self.store.search(spec)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn native_shared_create_reopen_and_provider_safe_reset() {
    let data =
        std::env::temp_dir().join(format!("native-worker-provider-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&data).expect("isolated native profile");
    let setup = super::prepare(&data.join("node")).expect("neutral prerequisites only");
    let ca = fs::read(setup.tls().ca_path()).expect("target CA");
    drop(setup);
    let bundled = super::prepare(&data.join("bundled/node")).expect("unselected desktop host");
    let mut ca_bundle = ca;
    ca_bundle.extend(fs::read(bundled.tls().ca_path()).expect("bundled CA"));
    drop(bundled);
    let ca = data.join("isolated-native-roots.pem");
    fs::write(&ca, ca_bundle).expect("isolated test-only root bundle");
    let cert_dir = data.join("empty-native-roots");
    fs::create_dir(&cert_dir).expect("isolated native root directory");
    let mut child = Command::new(std::env::current_exe().expect("native test executable"))
        .args([
            "--exact",
            "native_host::provider_lifecycle_tests::native_shared_create_child",
            "--ignored",
            "--nocapture",
        ])
        .env("OF_NATIVE_HOST_TEST_ROOT", &data)
        .env("SSL_CERT_FILE", ca)
        .env("SSL_CERT_DIR", cert_dir)
        .env("RUST_MIN_STACK", "16777216")
        .stdin(Stdio::null())
        .spawn()
        .expect("isolated provider/root child");
    let deadline = Instant::now() + Duration::from_secs(90);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill bounded child");
            child.wait().expect("reap child");
            panic!("native shared Create/reopen/reset exceeded unchanged 90-second bound");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    fs::remove_dir_all(data).expect("isolated cleanup");
    assert!(status.success(), "native shared worker lifecycle child");
}

async fn ready(host: &NativeHost) -> of_idp_client::models::CreateResult {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let status = host.status().await.expect("real shared status");
        if status.ready {
            return status;
        }
        assert!(
            !matches!(status.state.as_str(), "unavailable" | "unfinished"),
            "shared worker did not complete: {status:?}"
        );
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded owner completion"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "only the parent may supply isolated native roots and a mock protected provider"]
async fn native_shared_create_child() {
    let data =
        PathBuf::from(std::env::var_os("OF_NATIVE_HOST_TEST_ROOT").expect("isolated parent root"));
    let unavailable = Arc::new(AtomicBool::new(false));
    keyring_core::set_default_store(Arc::new(IsolatedProvider {
        store: keyring_core::mock::Store::new().expect("mock protected provider"),
        unavailable: unavailable.clone(),
    }));
    let setup = super::prepare(&data.join("node")).expect("retained neutral prerequisites");
    let state = fs::read_to_string(setup.admission_path()).expect("protected local code retrieval");
    let code = state.lines().nth(1).expect("host-generated code");
    let client = LocalTls::verification_client().expect("isolated OS roots");
    let mut host = NativeHost::start_with(setup, |_| Ok(()), || Ok(client.clone()))
        .await
        .expect("native one-listener host");
    let bundled_setup =
        super::prepare(&data.join("bundled/node")).expect("unselected desktop host");
    let mut bundled_host =
        NativeHost::start_with(bundled_setup, |_| Ok(()), LocalTls::verification_client)
            .await
            .expect("neutral bundled host");
    let base = host.base_url().to_owned();
    let body = json!({"operation_nonce":"0123456789abcdef0123456789abcdef", "administrator":"operator", "node":"Native Node", "password":"fixture-initial-password"});
    let response = client
        .post(format!("{base}/idp/setup/create"))
        .header("Authorization", format!("Setup {code}"))
        .json(&body)
        .send()
        .await
        .expect("same HTTPS Create used by web");
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
    drop(response);
    let status = ready(&host).await;
    let observed = host.observation().await;
    assert!(observed.failure.is_none());
    assert_eq!(
        observed
            .status
            .expect("typed shared observation")
            .login_client,
        status.login_client
    );
    fs::write(data.join("device-setup-ready"), "foreign-client")
        .expect("damaged preference marker");
    let damaged = host.observation().await;
    assert!(damaged.failure.is_none());
    assert!(damaged.status.expect("current shared readiness").ready);
    fs::remove_file(data.join("device-setup-ready")).expect("remove injected damaged marker");
    assert_eq!(
        bundled_host
            .status()
            .await
            .expect("unselected desktop status")
            .state,
        "awaiting_choice"
    );
    for owner in ["idp", "device", "storage", "endpoint"] {
        assert!(
            !data.join("bundled/node").join(owner).exists(),
            "addressing remote APIs must not create a desktop {owner}"
        );
    }
    let identity = fs::read(data.join("node/host.json")).expect("retained host identity");
    let journal = fs::read(data.join("node/endpoint.provisioning/intent.json"))
        .expect("single shared owner journal");
    let address: std::net::SocketAddr = base
        .strip_prefix("https://")
        .expect("HTTPS base")
        .parse()
        .expect("native address");
    assert!(
        tokio::net::TcpListener::bind(address).await.is_err(),
        "Create did not replace the listener"
    );
    let completed_retry = client
        .post(format!("{base}/idp/setup/create"))
        .json(&body)
        .send()
        .await
        .expect("normal authenticated completed retry");
    assert_eq!(completed_retry.status(), reqwest::StatusCode::OK);
    host.close()
        .await
        .expect("close worker and shared endpoint");
    let setup = super::prepare(&data.join("node")).expect("existing-only reopen");
    let mut reopened = NativeHost::start_with(setup, |_| Ok(()), LocalTls::verification_client)
        .await
        .expect("source-offline retained Node");
    assert_eq!(ready(&reopened).await.login_client, status.login_client);
    assert_eq!(reopened.base_url(), base);
    assert_eq!(
        fs::read(data.join("node/host.json")).expect("same host"),
        identity
    );
    assert_eq!(
        fs::read(data.join("node/endpoint.provisioning/intent.json")).expect("same owner journal"),
        journal
    );
    reopened
        .close()
        .await
        .expect("stopped host before explicit reset");
    unavailable.store(true, Ordering::SeqCst);
    assert!(
        crate::setup_reset::run(&data).await.is_err(),
        "provider refusal must prevent data deletion"
    );
    assert_eq!(
        fs::read(data.join("node/host.json")).expect("host retained after failure"),
        identity
    );
    assert_eq!(
        fs::read(data.join("node/endpoint.provisioning/intent.json"))
            .expect("references retained after failure"),
        journal
    );
    assert!(crate::setup_reset::pending(&data).expect("pending reset"));
    unavailable.store(false, Ordering::SeqCst);
    crate::setup_reset::run(&data)
        .await
        .expect("exact reset retry through mock protected provider");
    assert!(!data.join("node").exists());
    assert!(!crate::setup_reset::pending(&data).expect("reset finished"));
    assert_eq!(
        crate::setup_reset::revocations(&data)
            .expect("surviving public revocation target")
            .len(),
        1
    );
    bundled_host
        .close()
        .await
        .expect("close unselected desktop prerequisites");
}
