use std::{fs, io, path::PathBuf};

use unified_server::{LocalTls, SetupHost};

use super::NativeHost;

fn directory() -> PathBuf {
    std::env::temp_dir().join(format!("native-shared-host-{}", uuid::Uuid::new_v4()))
}

pub(crate) fn retained_identity(root: &std::path::Path) -> serde_json::Value {
    let host = unified_server::BootstrapConfig::open(root).expect("stopped retained host");
    let config = host.config();
    serde_json::json!({
        "listen": config.listen_addr, "issuer": config.idp.oauth2.issuer,
        "namespace": config.idp.key_namespace,
        "roots": [config.idp.data_dir, config.management.data_dir, config.device.data_dir, config.storage.data_dir, config.endpoint_data_dir.to_string_lossy().into_owned()],
        "applications": [config.idp.api_public_uri, config.management.api_public_uri, config.device.api_public_uri, config.storage.api_public_uri],
        "setup_attempted": host.setup_attempted(),
    })
}

fn fixture(root: &std::path::Path) -> (SetupHost, reqwest::Client) {
    let setup = super::prepare(root).expect("fresh neutral native host");
    let ca = fs::read(setup.tls().ca_path()).expect("isolated CA");
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .add_root_certificate(reqwest::Certificate::from_pem(&ca).expect("fixture CA"))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("fixture-only HTTPS verification");
    (setup, client)
}

#[tokio::test]
async fn native_host_keeps_one_listener_and_shared_routes_until_coordinated_close() {
    let root = directory();
    let (setup, client) = fixture(&root);
    let address = setup.local_addr();
    let mut host = NativeHost::start_with(
        setup,
        |_| panic!("already trusted: must not install"),
        || Ok(client.clone()),
    )
    .await
    .expect("injected trust installer, actual TLS listener");
    assert!(tokio::net::TcpListener::bind(address).await.is_err());
    let status = client
        .get(format!("{}/idp/setup/status", host.base_url()))
        .send()
        .await
        .expect("shared status")
        .json::<serde_json::Value>()
        .await
        .expect("status JSON");
    assert_eq!(status["state"], "awaiting_choice");
    assert_eq!(status["ready"], false);
    assert_eq!(
        status["owners"],
        serde_json::json!(["idp", "device", "storage"])
    );
    for path in ["idp", "device", "storage", "endpoint"] {
        assert!(
            !root.join(path).exists(),
            "startup must not provision {path}"
        );
    }
    fs::write(root.join("device-setup-ready"), "historical-marker")
        .expect("untrusted historical marker");
    let observed = host.observation().await;
    let serialized = serde_json::to_value(&observed).expect("public native observation");
    assert_eq!(serialized["status"]["state"], "awaiting_choice");
    assert_eq!(serialized["status"]["ready"], false);
    assert!(serialized.get("device_setup_required").is_none());
    assert!(serialized["failure"].is_null());
    assert!(!serialized.to_string().contains("historical-marker"));
    assert!(!serialized.to_string().contains("setup-admission"));
    let denied = client.post(format!("{}/idp/setup/create", host.base_url()))
        .json(&serde_json::json!({"operation_nonce":"0123456789abcdef0123456789abcdef", "administrator":"admin", "node":"Desktop", "password":"not-authorized"}))
        .send().await.expect("real shared Create route");
    assert_eq!(denied.status(), reqwest::StatusCode::UNAUTHORIZED);
    let denied = client.post(format!("{}/idp/setup/join", host.base_url()))
        .json(&serde_json::json!({"operation_nonce":"0123456789abcdef0123456789abcdef", "node":"Desktop", "peer":iroh::SecretKey::generate().public().to_string()}))
        .send().await.expect("real shared Join route");
    assert_eq!(denied.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        host.base_url(),
        LocalTls::api_base(address).expect("stable base")
    );
    host.close()
        .await
        .expect("join listener and worker cleanup");
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("listener released");
    drop(listener);
    let reopened = super::prepare(&root).expect("existing-only neutral reopen");
    assert_eq!(reopened.local_addr(), address);
    drop(reopened);
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[tokio::test]
async fn native_trust_failure_preserves_prerequisites_without_starting_domain_owners() {
    let root = directory();
    let (setup, _) = fixture(&root);
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .no_proxy()
        .build()
        .expect("untrusted fixture");
    let address = setup.local_addr();
    let ca = fs::read(setup.tls().ca_path()).expect("CA before refusal");
    assert!(
        NativeHost::start_with(
            setup,
            |_| Err(io::Error::other("consent denied")),
            || Ok(client.clone())
        )
        .await
        .is_err()
    );
    assert_eq!(
        fs::read(root.join("endpoint.tls/ca.pem")).expect("retained CA"),
        ca
    );
    assert!(!root.join("idp").exists());
    assert!(tokio::net::TcpListener::bind(address).await.is_ok());
    drop(super::prepare(&root).expect("exact retry after trust refusal"));
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn native_reopen_refuses_lost_host_and_tls_evidence_without_replacement() {
    for lost in [
        "host.redb",
        "setup-admission/state",
        "endpoint.tls/server.key",
    ] {
        let root = directory();
        let setup = super::prepare(&root).expect("fresh prerequisites");
        drop(setup);
        let host = retained_identity(&root);
        fs::remove_file(root.join(lost)).expect("inject lost evidence");
        let error = super::prepare(&root)
            .err()
            .expect("must refuse missing evidence");
        assert!(
            matches!(
                crate::onboarding::Failure::startup(&error),
                crate::onboarding::Failure::Repair
            ),
            "missing {lost} requires repair"
        );
        assert!(!root.join(lost).exists(), "must never regenerate {lost}");
        if lost != "host.redb" {
            assert_eq!(retained_identity(&root), host);
        }
        fs::remove_dir_all(root).expect("isolated cleanup");
    }
}

#[tokio::test]
async fn desktop_origin_is_explicit_and_does_not_admit_unrelated_browser_origins() {
    let root = directory();
    let (setup, client) = fixture(&root);
    let mut host = NativeHost::start_with(setup, |_| Ok(()), || Ok(client.clone()))
        .await
        .expect("isolated host");
    let url = format!("{}/idp/setup/status", host.base_url());
    assert_eq!(
        client
            .get(&url)
            .header("Origin", super::native_origin())
            .send()
            .await
            .expect("exact native origin")
            .status(),
        reqwest::StatusCode::OK
    );
    assert_eq!(
        client
            .get(&url)
            .header("Origin", "https://unrelated.example")
            .send()
            .await
            .expect("unrelated origin")
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    host.close().await.expect("close host and worker");
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[tokio::test]
async fn native_verification_failure_closes_the_listener_without_replacing_tls_or_owner_state() {
    let root = directory();
    let setup = super::prepare(&root).expect("fresh neutral prerequisites");
    let address = setup.local_addr();
    let ca = fs::read(setup.tls().ca_path()).expect("original CA");
    let untrusted = reqwest::Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .no_proxy()
        .build()
        .expect("empty root verifier");
    assert!(
        NativeHost::start_with(setup, |_| Ok(()), || Ok(untrusted.clone()))
            .await
            .is_err()
    );
    assert!(
        tokio::net::TcpListener::bind(address).await.is_ok(),
        "failed verification must join the listener"
    );
    assert!(!root.join("idp").exists());
    assert_eq!(
        fs::read(root.join("endpoint.tls/ca.pem")).expect("same CA"),
        ca
    );
    drop(super::prepare(&root).expect("exact retained retry, not CA regeneration"));
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[tokio::test]
async fn native_unknown_ca_requires_explicit_consent_and_retry_retains_identity() {
    let root = directory();
    let (setup, trusted) = fixture(&root);
    let ca = fs::read(setup.tls().ca_path()).expect("retained CA");
    let address = setup.local_addr();
    let untrusted = reqwest::Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .no_proxy()
        .build()
        .expect("empty roots");
    let error = NativeHost::start_with(
        setup,
        |_| Err(super::consent_required()),
        || Ok(untrusted.clone()),
    )
    .await
    .err()
    .expect("explicit consent required");
    assert!(matches!(
        crate::onboarding::Failure::startup(&error),
        crate::onboarding::Failure::TrustConsent
    ));
    assert!(tokio::net::TcpListener::bind(address).await.is_ok());
    let setup = super::prepare(&root).expect("exact retry");
    assert_eq!(fs::read(setup.tls().ca_path()).expect("same CA"), ca);
    let mut first = true;
    let mut host = NativeHost::start_with(
        setup,
        |_| Ok(()),
        || {
            let client = if first {
                untrusted.clone()
            } else {
                trusted.clone()
            };
            first = false;
            Ok(client)
        },
    )
    .await
    .expect("refresh verifier after consented installation");
    assert_eq!(
        host.base_url(),
        LocalTls::api_base(address).expect("same endpoint")
    );
    host.close().await.expect("join all workers");
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[tokio::test]
async fn close_during_startup_reaps_host_and_prevents_late_restart() {
    let lifecycle = crate::lifecycle::Lifecycle::default();
    let root = directory();
    let (setup, _) = fixture(&root);
    let address = setup.local_addr();
    let (operation, cancellation) = lifecycle.start().await.expect("initial startup ownership");
    let untrusted = reqwest::Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .no_proxy()
        .build()
        .expect("empty verifier");
    let (entered, waiting) = tokio::sync::oneshot::channel();
    let (release, paused) = std::sync::mpsc::channel();
    let startup = async {
        let result = NativeHost::start_cancellable(
            setup,
            move |_| {
                entered.send(()).expect("installer entered");
                paused.recv().expect("release owned installer");
                Ok(())
            },
            || Ok(untrusted.clone()),
            cancellation,
        )
        .await;
        assert!(result.is_err(), "cancelled startup cannot publish running");
        drop(operation);
    };
    let close = async {
        waiting.await.expect("startup in installer");
        let mut stop = Box::pin(lifecycle.stop(true));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut stop)
                .await
                .is_err(),
            "shutdown joins owned startup"
        );
        release.send(()).expect("finish installer");
        let _closed = stop.await;
        assert!(
            tokio::net::TcpListener::bind(address).await.is_ok(),
            "all listeners released before shutdown completes"
        );
    };
    tokio::join!(startup, close);
    assert!(
        lifecycle.start().await.is_none(),
        "close prohibits queued restart"
    );
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[tokio::test]
async fn reset_during_startup_cancels_and_joins_before_deleting_isolated_state() {
    let lifecycle = crate::lifecycle::Lifecycle::default();
    let data = directory();
    let (setup, client) = fixture(&data.join("node"));
    let address = setup.local_addr();
    let (operation, cancellation) = lifecycle.start().await.expect("startup ownership");
    let stop = lifecycle.stop(false);
    let startup = async {
        tokio::task::yield_now().await;
        let result = NativeHost::start_cancellable(
            setup,
            |_| panic!("reset cannot install trust"),
            || Ok(client.clone()),
            cancellation,
        )
        .await;
        assert!(result.is_err());
        drop(operation);
    };
    let reset = async {
        let _operation = stop.await;
        crate::setup_reset::run(&data)
            .await
            .expect("reset isolated neutral host only after startup joined");
        assert!(tokio::net::TcpListener::bind(address).await.is_ok());
        assert!(!data.join("node").exists());
    };
    tokio::join!(reset, startup);
    fs::remove_dir_all(data).expect("isolated cleanup");
}

#[tokio::test]
async fn occupied_retained_port_does_not_authorize_trust_installation() {
    let root = directory();
    let (setup, client) = fixture(&root);
    let ca = fs::read(setup.tls().ca_path()).expect("original CA");
    let listener = tokio::net::TcpListener::bind(setup.local_addr())
        .await
        .expect("occupied retained port");
    assert!(
        NativeHost::start_with(
            setup,
            |_| panic!("unrelated failure is not trust consent"),
            || Ok(client.clone())
        )
        .await
        .is_err()
    );
    assert_eq!(
        fs::read(root.join("endpoint.tls/ca.pem")).expect("retained CA"),
        ca
    );
    drop(listener);
    drop(super::prepare(&root).expect("exact retry"));
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[tokio::test]
async fn a_host_finishing_after_close_cannot_be_published_as_running() {
    let lifecycle = crate::lifecycle::Lifecycle::default();
    let root = directory();
    let (setup, client) = fixture(&root);
    let address = setup.local_addr();
    let (operation, cancellation) = lifecycle.start().await.expect("startup ownership");
    let host = NativeHost::start_cancellable(
        setup,
        |_| panic!("already trusted"),
        || Ok(client.clone()),
        cancellation.clone(),
    )
    .await
    .expect("verified host");
    let mut stop = Box::pin(lifecycle.stop(true));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), &mut stop)
            .await
            .is_err()
    );
    let mut slot = None;
    let mut rejected = lifecycle
        .publish_host(&cancellation, host, &mut slot)
        .expect("reject late host");
    assert!(slot.is_none());
    assert!(rejected.observation().await.status.is_none());
    rejected.close().await.expect("join rejected host");
    drop(operation);
    let _stopped = stop.await;
    assert!(tokio::net::TcpListener::bind(address).await.is_ok());
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn retained_host_lock_contention_is_retryable_without_replacing_identity() {
    let root = directory();
    let setup = super::prepare(&root).expect("fresh retained host");
    let address = setup.local_addr();
    drop(setup);
    let identity = retained_identity(&root);
    let setup = super::prepare(&root).expect("retained setup");
    let ca = fs::read(setup.tls().ca_path()).expect("retained CA");
    let error = super::prepare(&root)
        .err()
        .expect("exclusive retained host lock");
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(matches!(
        crate::onboarding::Failure::startup(&error),
        crate::onboarding::Failure::Retry
    ));
    drop(setup);
    let reopened = super::prepare(&root).expect("retry after releasing the lock");
    assert_eq!(reopened.local_addr(), address);
    assert!(
        fs::read(reopened.tls().ca_path()).expect("same CA") == ca,
        "CA changed"
    );
    drop(reopened);
    assert_eq!(retained_identity(&root), identity);
    fs::remove_dir_all(root).expect("isolated fixture cleanup");
}

#[tokio::test]
async fn failed_reference_recovery_publishes_retry_and_preserves_retained_host() {
    let data = directory();
    fs::create_dir_all(&data).expect("isolated application data");
    let root = data.join("node");
    let (setup, _) = fixture(&root);
    drop(setup);
    let identity = retained_identity(&root);
    let (setup, client) = fixture(&root);
    let ca = fs::read(setup.tls().ca_path()).expect("retained CA");
    let host = NativeHost::start_with(setup, |_| panic!("trusted"), || Ok(client.clone()))
        .await
        .expect("healthy host before maintenance");
    let address = host.base_url().to_owned();
    let host = tokio::sync::Mutex::new(Some(host));
    let failure = tokio::sync::Mutex::new(None);
    fs::write(
        data.join("setup-reference-backup.json"),
        "invalid recovery evidence",
    )
    .expect("inject failed recovery");
    let lifecycle = crate::lifecycle::Lifecycle::default();
    assert!(
        lifecycle
            .recover_references(&data, &host, &failure)
            .await
            .is_err()
    );
    assert!(host.lock().await.is_none());
    assert!(matches!(
        *failure.lock().await,
        Some(crate::onboarding::Failure::Retry)
    ));
    assert_eq!(retained_identity(&root), identity);
    let (setup, client) = fixture(&root);
    assert_eq!(fs::read(setup.tls().ca_path()).expect("same CA"), ca);
    let mut retry =
        NativeHost::start_with(setup, |_| panic!("trusted retry"), || Ok(client.clone()))
            .await
            .expect("explicit retry without replacement");
    assert_eq!(retry.base_url(), address);
    retry.close().await.expect("close retried host");
    fs::remove_dir_all(data).expect("isolated cleanup");
}

#[test]
fn corrupt_retained_identity_requires_repair_without_replacement() {
    let root = directory();
    drop(super::prepare(&root).expect("fresh host"));
    let corrupt = b"not a retained host identity";
    fs::write(root.join("host.redb"), corrupt).expect("inject corrupt identity");
    let error = super::prepare(&root)
        .err()
        .expect("corrupt identity refused");
    assert!(matches!(
        crate::onboarding::Failure::startup(&error),
        crate::onboarding::Failure::Repair
    ));
    assert!(
        fs::read(root.join("host.redb")).expect("no replacement") == corrupt,
        "corrupt database changed"
    );
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[test]
fn corrupt_retained_admission_requires_repair_without_replacement() {
    let root = directory();
    drop(super::prepare(&root).expect("fresh host"));
    let identity = retained_identity(&root);
    let corrupt = b"not retained admission";
    fs::write(root.join("setup-admission/state"), corrupt).expect("inject corrupt admission");
    let error = super::prepare(&root)
        .err()
        .expect("corrupt admission refused");
    assert!(matches!(
        crate::onboarding::Failure::startup(&error),
        crate::onboarding::Failure::Repair
    ));
    assert_eq!(
        fs::read(root.join("setup-admission/state")).expect("no replacement"),
        corrupt
    );
    assert_eq!(retained_identity(&root), identity);
    fs::remove_dir_all(root).expect("isolated cleanup");
}
