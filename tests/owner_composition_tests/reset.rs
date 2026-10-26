use std::{
    any::Any,
    collections::HashMap,
    fs, io,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use api::{IndependentCreateIntent, LocalTls};
use idp_model::model::Id;
use idp_service::repo::{PrivateKeyKeyringRepo, RawKeyringRepo};
use keyring_core::{
    Entry,
    api::{Credential, CredentialApi, CredentialStoreApi},
};
use serde_json::Value;
use tokio::time::{Duration, Instant, sleep};
use tokio_util::sync::CancellationToken;

use crate::owner_composition_tests::create::{setup, setup_code};

pub(super) fn prepare_trust_removal(root: &Path) {
    let command = root.join("commands/pkexec");
    fs::write(&command, "#!/bin/sh\nexit 0\n").expect("mock native consent and trust refresh only");
    fs::set_permissions(command, fs::Permissions::from_mode(0o700))
        .expect("executable isolated pkexec mock");
}

pub(super) fn prepare_interrupted(root: &Path, address: SocketAddr) -> Vec<u8> {
    let interrupted = root.join("reset-interrupted");
    fs::create_dir(&interrupted).expect("isolated interrupted-attempt root");
    fs::set_permissions(&interrupted, fs::Permissions::from_mode(0o700))
        .expect("private interrupted root");
    let host = setup(&interrupted, "storage", address);
    fs::read(host.tls().ca_path())
        .expect("isolated interrupted CA added only to test native-root bundle")
}

pub(super) async fn qualify_interrupted(root: &Path, address: SocketAddr, provider: &Provider) {
    let interrupted = root.join("reset-interrupted");
    let domain = interrupted.join("storage");
    let before = provider.snapshot();
    let shutdown = CancellationToken::new();
    let mut config = storage_server::AppConfig {
        data_dir: domain.to_string_lossy().into_owned(),
        ..Default::default()
    };
    config.server.prefix = Some("/storage".into());
    let server = tokio::spawn(storage_server::serve_independent_create(
        Arc::new(config),
        setup(&interrupted, "storage", address),
        shutdown.clone(),
    ));
    let http = LocalTls::verification_client().expect("isolated native verification");
    let base = format!("https://{address}/storage");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if http
            .get(format!("{base}/independent/setup/status"))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "interrupted fixture listener starts within existing child bound"
        );
        sleep(Duration::from_millis(10)).await;
    }
    let intent = IndependentCreateIntent {
        purpose: "create".into(),
        operation_nonce: "abcdef0123456789abcdef0123456789".into(),
        administrator: "operator".into(),
        node: "Interrupted reset target".into(),
        idp: "https://127.0.0.1:1/idp".into(),
        device: "https://127.0.0.1:2/device".into(),
        storage: base.clone(),
        issuer: "https://127.0.0.1:1/idp".into(),
        peer: None,
    };
    provider.unavailable.store(true, Ordering::SeqCst);
    let response = http
        .post(format!("{base}/independent/setup/create"))
        .header(
            "Authorization",
            format!("Setup {}", setup_code(&interrupted, "storage")),
        )
        .json(&intent)
        .send()
        .await
        .expect("actual accepted interrupted attempt");
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
    let worker_path = interrupted.join("storage.setup-host/worker.json");
    let deadline = Instant::now() + Duration::from_secs(3);
    let worker = loop {
        if let Ok(bytes) = fs::read(&worker_path) {
            let worker: Value =
                serde_json::from_slice(&bytes).expect("atomic current worker record");
            if worker["attempted"][0] == true {
                break bytes;
            }
        }
        assert!(
            Instant::now() < deadline,
            "actual secret attempt is fenced before provider access"
        );
        sleep(Duration::from_millis(10)).await;
    };
    assert!(
        !domain.exists(),
        "secret-attempt failure precedes domain creation"
    );
    assert!(
        reset("storage", &domain, true).await.is_err(),
        "even a failed worker retains the setup-host lock until stopped"
    );
    shutdown.cancel();
    server
        .await
        .expect("interrupted owner task")
        .expect("stopped interrupted owner");
    assert!(
        reset("storage", &domain, true).await.is_err(),
        "missing provider preserves attempted evidence"
    );
    assert_eq!(
        fs::read(&worker_path).expect("exact retry evidence"),
        worker
    );
    provider.unavailable.store(false, Ordering::SeqCst);
    reset("storage", &domain, true)
        .await
        .expect("explicit reset of actual missing/attempted protected material");
    assert_eq!(
        provider.snapshot(),
        before,
        "reset never regenerates missing attempted keys or touches unrelated protected state"
    );
    assert!(!interrupted.join("storage.setup-host").exists());
}

pub(super) struct Provider {
    store: Arc<keyring_core::mock::Store>,
    unavailable: AtomicBool,
    fail_endpoint_delete: Arc<AtomicBool>,
    pub(super) fail_node_readback: Arc<AtomicBool>,
}

impl Provider {
    pub(super) fn install() -> Arc<Self> {
        let provider = Arc::new(Self {
            store: keyring_core::mock::Store::new().expect("isolated protected provider"),
            unavailable: AtomicBool::new(false),
            fail_endpoint_delete: Arc::new(AtomicBool::new(false)),
            fail_node_readback: Arc::new(AtomicBool::new(false)),
        });
        PrivateKeyKeyringRepo::new_with_store("independent-reset-qualification", provider.clone());
        provider
    }

    fn snapshot(&self) -> HashMap<(String, String), Vec<u8>> {
        self.store
            .search(&HashMap::new())
            .expect("isolated provider inventory")
            .into_iter()
            .filter_map(|entry| {
                entry.get_secret().ok().map(|secret| {
                    (
                        entry.get_specifiers().expect("exact provider specifier"),
                        secret,
                    )
                })
            })
            .collect()
    }
}

impl CredentialStoreApi for Provider {
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
    ) -> keyring_core::Result<Entry> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(keyring_core::Error::PlatformFailure(Box::new(
                io::Error::other("isolated provider unavailable"),
            )));
        }
        let entry = CredentialStoreApi::build(&*self.store, service, user, None)?;
        if user.contains(":node:")
            && entry.get_secret().is_ok()
            && self.fail_node_readback.swap(false, Ordering::SeqCst)
        {
            return Err(keyring_core::Error::NoEntry);
        }
        Ok(Entry::new_with_credential(Arc::new(FaultCredential {
            entry,
            endpoint: user == "endpoint",
            fail_delete: self.fail_endpoint_delete.clone(),
        })))
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<Entry>> {
        self.store.search(spec)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct FaultCredential {
    entry: Entry,
    endpoint: bool,
    fail_delete: Arc<AtomicBool>,
}
impl CredentialApi for FaultCredential {
    fn set_secret(&self, value: &[u8]) -> keyring_core::Result<()> {
        self.entry.set_secret(value)
    }
    fn get_secret(&self) -> keyring_core::Result<Vec<u8>> {
        self.entry.get_secret()
    }
    fn delete_credential(&self) -> keyring_core::Result<()> {
        if self.endpoint && self.fail_delete.load(Ordering::SeqCst) {
            return Err(keyring_core::Error::PlatformFailure(Box::new(
                io::Error::other("isolated protected delete failure"),
            )));
        }
        self.entry.delete_credential()
    }
    fn get_credential(&self) -> keyring_core::Result<Option<Arc<Credential>>> {
        self.entry.get_secret()?;
        Ok(None)
    }
    fn get_specifiers(&self) -> Option<(String, String)> {
        self.entry.get_specifiers()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

async fn reset(owner: &str, domain: &Path, trust: bool) -> io::Result<()> {
    let data_dir = domain.to_string_lossy().into_owned();
    match owner {
        "idp" => {
            idp_server::reset_independent(
                &idp_server::AppConfig {
                    data_dir,
                    ..Default::default()
                },
                trust,
            )
            .await
        }
        "device" => {
            device_server::reset_independent(
                &device_server::AppConfig {
                    data_dir,
                    ..Default::default()
                },
                trust,
            )
            .await
        }
        "storage" => {
            storage_server::reset_independent(
                &storage_server::AppConfig {
                    data_dir,
                    ..Default::default()
                },
                trust,
            )
            .await
        }
        _ => Err(io::Error::other("unexpected owner")),
    }
}

/// Runs inside the existing bounded child after actual HTTPS Create/Join and offline reopen.
pub(super) async fn qualify_completed(root: &Path, provider: &Provider, issuer: &str) {
    let commands = std::env::var_os("PATH").expect("isolated trust PATH");
    assert_eq!(
        fs::read(Path::new(&commands).join("pkexec")).expect("mock native consent command"),
        b"#!/bin/sh\nexit 0\n"
    );
    assert_eq!(
        fs::read(Path::new(&commands).join("trust")).expect("mock trust command"),
        b"#!/bin/sh\nexit 0\n"
    );
    if std::env::var("OF_INDEPENDENT_UNSUPPORTED_RESET").as_deref() == Ok("1") {
        qualify_unsupported_records(root, provider).await;
    }
    let device = root.join("device");
    let worker_path = root.join("device.setup-host/worker.json");
    let bytes = fs::read(&worker_path).expect("actual completed worker");
    let worker: Value = serde_json::from_slice(&bytes).expect("current worker schema");
    assert_eq!(worker["complete"], true);
    let client = worker["client"].as_str().expect("exact consumer slot");
    let scope = format!(
        "independent-setup:{}",
        root.join("device.setup-host").display()
    );
    let local = RawKeyringRepo::new(&scope).expect("mock provider");
    local
        .store("", "", "not-a-worker-slot", b"unrelated local slot")
        .expect("unrelated local material");
    let foreign = RawKeyringRepo::new("independent-setup:unrelated-root").expect("foreign scope");
    foreign
        .store("", "", client, b"same slot in foreign scope")
        .expect("foreign material");
    let idp_before =
        fs::read(root.join("idp.setup-host/worker.json")).expect("other owner evidence");
    let storage_before =
        fs::read(root.join("storage.setup-host/worker.json")).expect("other owner evidence");
    let idp_worker: Value = serde_json::from_slice(&idp_before).expect("actual IdP references");
    let storage_worker: Value =
        serde_json::from_slice(&storage_before).expect("actual Storage references");
    let scope_for = |owner| {
        format!(
            "independent-setup:{}",
            root.join(format!("{owner}.setup-host")).display()
        )
    };
    let mut removed = vec![
        (scope.clone(), client.to_owned()),
        (scope.clone(), "endpoint".into()),
        (
            scope_for("idp"),
            idp_worker["client"]
                .as_str()
                .expect("IdP consumer")
                .to_owned(),
        ),
        (
            issuer.to_owned(),
            format!(
                "{}:node:{}:{}:m",
                idp_server::AppConfig::default().key_namespace,
                idp_worker["member"].as_str().expect("IdP member"),
                idp_worker["key"].as_str().expect("IdP signer")
            ),
        ),
    ];
    removed.extend(
        storage_worker["clients"]
            .as_array()
            .expect("Storage references")
            .iter()
            .map(|id| {
                (
                    scope_for("storage"),
                    id.as_str().expect("Storage consumer").to_owned(),
                )
            }),
    );
    let unrelated = provider
        .snapshot()
        .into_iter()
        .filter(|(reference, _)| !removed.contains(reference))
        .collect::<HashMap<_, _>>();
    assert_eq!(
        worker["version"], 2,
        "cleanup uses the current worker contract"
    );
    let before_refusal = provider.snapshot();
    let error = reset("device", &device, false)
        .await
        .expect_err("trust removal requires explicit consent");
    assert!(error.to_string().contains("--remove-os-trust"));
    provider.unavailable.store(true, Ordering::SeqCst);
    let error = reset("device", &device, true)
        .await
        .expect_err("provider absence is not successful cleanup");
    assert!(error.to_string().contains("isolated provider unavailable"));
    provider.unavailable.store(false, Ordering::SeqCst);
    assert_eq!(provider.snapshot(), before_refusal);
    assert_eq!(fs::read(&worker_path).expect("preserved worker"), bytes);
    assert!(device.exists());
    provider.fail_endpoint_delete.store(true, Ordering::SeqCst);
    let error = reset("device", &device, true)
        .await
        .expect_err("actual provider deletion failure");
    assert!(
        error
            .to_string()
            .contains("isolated protected delete failure")
    );
    assert!(
        local
            .load("", "", client)
            .expect("partially deleted client")
            .is_none()
    );
    assert!(
        local
            .load("", "", "endpoint")
            .expect("failed endpoint deletion")
            .is_some()
    );
    assert_eq!(
        fs::read(&worker_path).expect("preserved retry evidence"),
        bytes
    );
    assert!(root.join("device.setup-host/tls/ca.pem").exists());
    provider.fail_endpoint_delete.store(false, Ordering::SeqCst);
    reset("device", &device, true)
        .await
        .expect("exact partial cleanup retry");
    assert!(!device.exists());
    assert!(!root.join("device.setup-host").exists());
    assert!(
        local
            .load("", "", "endpoint")
            .expect("verified protected removal")
            .is_none()
    );
    assert_eq!(
        local
            .load("", "", "not-a-worker-slot")
            .expect("unrelated local slot"),
        Some(b"unrelated local slot".to_vec())
    );
    assert_eq!(
        foreign
            .load("", "", client)
            .expect("foreign slot preserved"),
        Some(b"same slot in foreign scope".to_vec())
    );
    assert_eq!(
        fs::read(root.join("idp.setup-host/worker.json")).expect("IdP untouched"),
        idp_before
    );
    assert_eq!(
        fs::read(root.join("storage.setup-host/worker.json")).expect("Storage untouched"),
        storage_before
    );
    let before = provider.snapshot();
    reset("idp", &root.join("idp"), true)
        .await
        .expect("actual IdP protected inventory reset");
    let after = provider.snapshot();
    assert!(
        after.len() < before.len(),
        "IdP signer/consumer material was actually deleted"
    );
    let blocked = root.join("storage/blocked-reset-cleanup");
    fs::create_dir(&blocked).expect("isolated cleanup-failure fixture");
    fs::write(blocked.join("retained"), b"local data").expect("local content");
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o000))
        .expect("inject local cleanup failure");
    assert!(
        reset("storage", &root.join("storage"), true).await.is_err(),
        "local cleanup failure remains retryable after provider/trust removal"
    );
    assert!(root.join("storage.setup-host/worker.json").exists());
    assert!(root.join("storage.setup-reset.json").exists());
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700))
        .expect("restore only isolated cleanup permission");
    reset("storage", &root.join("storage"), true)
        .await
        .expect("actual Storage reset retries local cleanup without recreating secrets");
    let after = provider.snapshot();
    for (reference, secret) in unrelated {
        assert_eq!(
            after.get(&reference),
            Some(&secret),
            "unrelated/source provider slot changed: {reference:?}"
        );
    }
    for reference in removed {
        assert!(
            !after.contains_key(&reference),
            "captured slot survived reset: {reference:?}"
        );
    }
    for owner in ["idp", "device", "storage"] {
        assert!(!root.join(owner).exists());
        assert!(!root.join(format!("{owner}.setup-host")).exists());
        assert!(!root.join(format!("{owner}.setup-reset.json")).exists());
    }
}

async fn qualify_unsupported_records(root: &Path, provider: &Provider) {
    for owner in ["idp", "device"] {
        let domain = root.join(owner);
        let directory = root.join(format!("{owner}.setup-host"));
        let path = directory.join("worker.json");
        let current = fs::read(&path).expect("completed current record");
        let record: Value = serde_json::from_slice(&current).expect("current worker");
        assert_eq!(record["version"], 2);
        let host = fs::read(directory.join("host.json")).expect("retained host");
        let ca = fs::read(directory.join("tls/ca.pem")).expect("retained CA");
        let sentinel = domain.join("reset-refusal-sentinel");
        fs::write(&sentinel, b"retained local data").expect("isolated local data");
        let before = provider.snapshot();
        let mut unsupported = record.clone();
        unsupported["version"] = serde_json::json!(if owner == "device" { 1 } else { 255 });
        let mut historical = record;
        historical["role"] = serde_json::json!(Id::now_v7());
        historical["permissions"] = serde_json::json!([]);
        for (bytes, reason) in [
            (
                serde_json::to_vec(&unsupported).expect("unsupported fixture"),
                if owner == "device" {
                    "unsupported Device reset record"
                } else {
                    "invalid retained IdP reset inventory"
                },
            ),
            (
                serde_json::to_vec(&historical).expect("unknown-field fixture"),
                "unknown field",
            ),
            (b"{malformed".to_vec(), "key must be a string"),
        ] {
            let journal = root.join(format!("{owner}.setup-reset.json"));
            assert!(
                !journal.exists(),
                "each negative fixture starts without a journal"
            );
            fs::write(&path, &bytes).expect("isolated unsupported evidence");
            let settings: Value = serde_json::from_slice(&host).expect("retained host settings");
            let address = settings["address"]
                .as_str()
                .expect("retained listener")
                .parse()
                .expect("address");
            let setup = setup(root, owner, address);
            let shutdown = CancellationToken::new();
            let result = if owner == "idp" {
                let mut config = idp_server::AppConfig {
                    data_dir: domain.to_string_lossy().into_owned(),
                    ..Default::default()
                };
                config.server.prefix = Some("/idp".into());
                idp_server::serve_independent_create(Arc::new(config), setup, shutdown).await
            } else {
                let mut config = device_server::AppConfig {
                    data_dir: domain.to_string_lossy().into_owned(),
                    ..Default::default()
                };
                config.server.prefix = Some("/device".into());
                device_server::serve_independent_create(Arc::new(config), setup, shutdown).await
            };
            assert!(
                result.is_err(),
                "ordinary startup refuses unsupported {owner} evidence"
            );
            let error = reset(owner, &domain, true)
                .await
                .expect_err("unsupported/malformed evidence must refuse before deletion");
            assert!(error.to_string().contains(reason), "{owner}: {error}");
            assert_eq!(provider.snapshot(), before, "protected material preserved");
            assert_eq!(fs::read(&path).expect("worker preserved"), bytes);
            assert_eq!(
                fs::read(directory.join("host.json")).expect("host preserved"),
                host
            );
            assert_eq!(
                fs::read(directory.join("tls/ca.pem")).expect("CA preserved"),
                ca
            );
            assert_eq!(
                fs::read(&sentinel).expect("local data preserved"),
                b"retained local data"
            );
            if journal.exists() {
                let retained: Value =
                    serde_json::from_slice(&fs::read(&journal).expect("refused reset inventory"))
                        .expect("reset journal");
                assert_eq!(retained["phase"], "secrets");
                assert_eq!(
                    retained["worker"],
                    serde_json::from_slice::<Value>(&bytes).expect("captured evidence")
                );
                let journal_bytes = fs::read(&journal).expect("retained refused inventory");
                fs::write(&path, &current)
                    .expect("inject a conflicting worker against retained reset");
                let error = reset(owner, &domain, true)
                    .await
                    .expect_err("current worker cannot overwrite retained reset references");
                assert!(error.to_string().contains("retained setup changed"));
                assert_eq!(
                    fs::read(&journal).expect("inventory unchanged"),
                    journal_bytes
                );
                assert_eq!(provider.snapshot(), before);
                // Discard only this injected fixture's journal, not a production recovery path.
                fs::remove_file(&journal).expect("discard isolated negative fixture journal");
            }
            fs::write(&path, &current).expect("restore exact current test fixture, not migration");
        }
        fs::remove_file(sentinel).expect("remove only isolated sentinel");
    }
}
