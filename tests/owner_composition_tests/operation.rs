use std::{
    fs,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use api::{IndependentCreateIntent, LocalTls};
use device_service::DeviceRepo;
use device_service::replica::DbDeviceRepo;
use idp_model::model::Id;
use reqwest::{Client, StatusCode};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::owner_composition_tests::create::{
    current_status, normal_use, seed_endpoint_negatives, setup, setup_code, start, wait,
};

#[test]
fn production_independent_signer_free_operation_and_delayed_authority_reopen() {
    let root = std::env::temp_dir().join(format!("independent-operation-{}", Id::now_v7()));
    fs::create_dir(&root).expect("isolated roots");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private roots");
    let mut addresses = Vec::new();
    let mut bundle = Vec::new();
    for owner in ["idp", "device", "storage"] {
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve");
        let address = reserved.local_addr().expect("address");
        let host = setup(&root, owner, address);
        bundle.extend(fs::read(host.tls().ca_path()).expect("owner CA"));
        addresses.push(address);
    }
    fs::write(
        root.join("addresses.json"),
        serde_json::to_vec(&addresses).expect("JSON"),
    )
    .expect("addresses");
    fs::write(root.join("roots.pem"), bundle).expect("native roots");
    fs::create_dir(root.join("empty-roots")).expect("isolated roots");
    fs::create_dir(root.join("commands")).expect("isolated commands");
    fs::write(root.join("commands/trust"), "#!/bin/sh\nexit 0\n").expect("mock trust");
    fs::set_permissions(
        root.join("commands/trust"),
        fs::Permissions::from_mode(0o700),
    )
    .expect("executable");
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "owner_composition_tests::operation::operation_child",
            "--ignored",
            "--nocapture",
        ])
        .env("OF_OPERATION_ROOT", &root)
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
            panic!("independent operation exceeded 90 seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    fs::remove_dir_all(root).expect("isolated cleanup");
    assert!(
        status.success(),
        "actual independent normal-operation child"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "parent supplies private native roots and mock secure provider/trust"]
async fn operation_child() {
    let root = PathBuf::from(std::env::var_os("OF_OPERATION_ROOT").expect("parent roots"));
    crate::support::timing::capture_setup_timings();
    let _provider = crate::owner_composition_tests::reset::Provider::install();
    let addresses: Vec<SocketAddr> =
        serde_json::from_slice(&fs::read(root.join("addresses.json")).expect("addresses"))
            .expect("JSON");
    let intent = IndependentCreateIntent {
        purpose: "create".into(),
        operation_nonce: "0123456789abcdef0123456789abcdef".into(),
        administrator: "operator".into(),
        node: "Independent operation".into(),
        idp: format!("https://{}/idp", addresses[0]),
        device: format!("https://{}/device", addresses[1]),
        storage: format!("https://{}/storage", addresses[2]),
        issuer: format!("https://{}/idp", addresses[0]),
        peer: None,
    };
    let http = LocalTls::verification_client().expect("native HTTPS");
    let shutdown = CancellationToken::new();
    let servers = start(&root, &addresses, &shutdown).await;
    wait(&http, &intent, false).await;
    for owner in ["idp", "device", "storage"] {
        let mut body = serde_json::to_value(&intent).expect("intent");
        if owner == "idp" {
            body["password"] = serde_json::json!("protected-password");
        }
        assert_eq!(
            http.post(format!(
                "{}/independent/setup/create",
                intent.base(owner).expect("base")
            ))
            .header(
                "Authorization",
                format!("Setup {}", setup_code(&root, owner))
            )
            .json(&body)
            .send()
            .await
            .expect("accepted Create")
            .status(),
            StatusCode::ACCEPTED
        );
    }
    wait(&http, &intent, true).await;
    let (_, management_token, storage_token) = normal_use(&http, &intent).await;
    shutdown.cancel();
    for server in servers {
        server.await.expect("join").expect("shutdown");
    }

    delayed_authority_reopen(&root, &addresses, &intent, &http).await;

    let endpoints = seed_endpoint_negatives(&root).await;
    let endpoint = &endpoints[0].0;
    let device = {
        let engine = Arc::new(
            db::Database::open(root.join("device/devices.redb")).expect("stopped Management"),
        );
        DbDeviceRepo::new(engine.client())
            .find_approved_by_public_key(endpoint)
            .await
            .expect("lookup")
            .expect("approved")
            .id
    };
    // The fixture creates this endpoint through Management only, never through IdP enrollment.
    let idp_worker = fs::read(root.join("idp.setup-host/worker.json")).expect("retained signer");
    {
        let engine = Arc::new(db::Database::open(root.join("idp/idp.redb")).expect("stopped IdP"));
        let signers = idp_service::replica::DbReplicaSignerRepo::new(engine.client())
            .trusted_signers(
                &intent.issuer,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_secs() as i64,
            )
            .await
            .expect("current signer trust");
        assert!(
            !signers.is_empty(),
            "valid issuer still has token-signing trust"
        );
        assert!(
            signers
                .iter()
                .all(|(member, _)| member.endpoint_id != *endpoint),
            "approved endpoint has no IdP signer membership"
        );
    }
    let retained = setup(&root, "storage", addresses[2]);
    let tls = retained.tls().clone();
    drop(retained);
    let shutdown = CancellationToken::new();
    let servers = start(&root, &addresses, &shutdown).await;
    wait(&http, &intent, true).await;
    let operation_root = root.join("operation-storage");
    let operation_address = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("reserve")
        .local_addr()
        .expect("address");

    let mut runtime = operation_runtime(
        &root,
        &operation_root,
        &intent,
        endpoint,
        shutdown.child_token(),
    );
    let listener = tokio::net::TcpListener::bind(operation_address)
        .await
        .expect("operation listener");
    let stop = CancellationToken::new();
    let connections = Arc::new(AtomicUsize::new(0));
    let server = tokio::spawn({
        let listener = CountedListener(tls.listener(listener), connections.clone());
        let router = runtime.router();
        let stop = stop.clone();
        async move {
            axum::serve(listener, router.into_make_service())
                .with_graceful_shutdown(stop.cancelled_owned())
                .await
        }
    });
    let url = format!("https://{operation_address}/storage/databases");
    assert_eq!(operation(&http, &url, None).await, StatusCode::UNAUTHORIZED);
    for allowed in [false, true, false] {
        restrict(&http, &intent, &management_token, device, allowed).await;
        let status = operation(&http, &url, Some(&storage_token)).await;
        assert_eq!(
            status,
            if allowed {
                StatusCode::OK
            } else {
                StatusCode::UNAUTHORIZED
            }
        );
    }
    assert_eq!(
        connections.load(Ordering::SeqCst),
        1,
        "learned policy denial on the same TLS connection"
    );
    stop.cancel();
    server.await.expect("listener join").expect("listener stop");
    runtime.shutdown().await.expect("runtime stop");
    runtime = operation_runtime(
        &root,
        &operation_root,
        &intent,
        endpoint,
        shutdown.child_token(),
    );
    let connections = Arc::new(AtomicUsize::new(0));
    let listener = CountedListener(
        tls.listener(
            tokio::net::TcpListener::bind(operation_address)
                .await
                .expect("retained listener"),
        ),
        connections.clone(),
    );
    let stop = CancellationToken::new();
    let server = tokio::spawn({
        let router = runtime.router();
        let stop = stop.clone();
        async move {
            axum::serve(listener, router.into_make_service())
                .with_graceful_shutdown(stop.cancelled_owned())
                .await
        }
    });
    assert_eq!(
        operation(&http, &url, Some(&storage_token)).await,
        StatusCode::UNAUTHORIZED
    );
    restrict(&http, &intent, &management_token, device, true).await;
    assert_eq!(
        operation(&http, &url, Some(&storage_token)).await,
        StatusCode::OK
    );
    assert_eq!(
        http.delete(format!("{}/devices/{device}", intent.device))
            .bearer_auth(&management_token)
            .send()
            .await
            .expect("learned endpoint revocation")
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        operation(&http, &url, Some(&storage_token)).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        connections.load(Ordering::SeqCst),
        1,
        "learned endpoint denial on the same TLS connection"
    );
    stop.cancel();
    server.await.expect("join").expect("stop");
    runtime.shutdown().await.expect("shutdown");
    let runtime = operation_runtime(
        &root,
        &operation_root,
        &intent,
        endpoint,
        shutdown.child_token(),
    );
    let listener = tls.listener(
        tokio::net::TcpListener::bind(operation_address)
            .await
            .expect("revoked reopen listener"),
    );
    let stop = CancellationToken::new();
    let server = tokio::spawn({
        let router = runtime.router();
        let stop = stop.clone();
        async move {
            axum::serve(listener, router.into_make_service())
                .with_graceful_shutdown(stop.cancelled_owned())
                .await
        }
    });
    assert_eq!(
        operation(&http, &url, Some(&storage_token)).await,
        StatusCode::UNAUTHORIZED
    );
    stop.cancel();
    server
        .await
        .expect("join revoked reopen")
        .expect("stop revoked reopen");
    runtime.shutdown().await.expect("shutdown revoked reopen");
    shutdown.cancel();
    for server in servers {
        server.await.expect("join").expect("shutdown");
    }
    assert_eq!(
        fs::read(root.join("idp.setup-host/worker.json")).expect("unchanged signer"),
        idp_worker
    );
}

struct CountedListener<L>(L, Arc<AtomicUsize>);

impl<L: axum::serve::Listener> axum::serve::Listener for CountedListener<L> {
    type Io = L::Io;
    type Addr = L::Addr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let accepted = self.0.accept().await;
        self.1.fetch_add(1, Ordering::SeqCst);
        accepted
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.0.local_addr()
    }
}

async fn operation(http: &Client, url: &str, token: Option<&str>) -> StatusCode {
    let request = http.get(url);
    let request = match token {
        Some(token) => request.bearer_auth(token),
        None => request,
    };
    let response = request
        .send()
        .await
        .expect("actual normal Storage operation");
    let status = response.status();
    let body = response
        .bytes()
        .await
        .expect("consume response on retained connection");
    if status == StatusCode::OK {
        assert_eq!(
            serde_json::from_slice::<Value>(&body).expect("actual catalog JSON"),
            serde_json::json!([])
        );
    }
    status
}

fn operation_runtime(
    root: &Path,
    data: &Path,
    intent: &IndependentCreateIntent,
    endpoint: &str,
    shutdown: CancellationToken,
) -> storage_server::StorageRuntime {
    let record: Value = serde_json::from_slice(
        &fs::read(root.join("storage.setup-host/worker.json")).expect("service clients"),
    )
    .expect("JSON");
    let scope = format!(
        "independent-setup:{}",
        root.join("storage.setup-host").display()
    );
    let client = |index: usize| {
        let id = record["clients"][index].as_str().expect("Client");
        (
            id,
            idp_service::setup_credentials::load_setup_secret(&scope, id)
                .expect("protected secret"),
        )
    };
    let (id, secret) = client(0);
    let idp = storage_server::IdpClient::new(&intent.idp, id, &secret, &intent.issuer, &intent.idp)
        .expect("IdP client");
    let (id, secret) = client(1);
    let device = storage_server::DeviceClient::new(
        &intent.device,
        &intent.idp,
        id,
        &secret,
        &intent.issuer,
        &intent.storage,
    )
    .expect("Management client");
    storage_server::build_http_runtime(
        storage_server::RouterState::new(&intent.storage)
            .with_idp_client(idp)
            .with_device_client(device),
        data,
        "/storage",
        endpoint.parse().expect("approved endpoint"),
        shutdown,
    )
    .expect("production HTTP runtime")
}

async fn restrict(
    http: &Client,
    intent: &IndependentCreateIntent,
    token: &str,
    device: Id,
    allowed: bool,
) {
    assert_eq!(
        http.put(format!("{}/devices/{device}/restriction", intent.device))
            .bearer_auth(token)
            .json(&serde_json::json!({"adminAllowed":allowed}))
            .send()
            .await
            .expect("current exact policy")
            .status(),
        StatusCode::NO_CONTENT
    );
}

async fn delayed_authority_reopen(
    root: &Path,
    addresses: &[SocketAddr],
    intent: &IndependentCreateIntent,
    http: &Client,
) {
    let evidence: Vec<_> = ["idp", "device", "storage"]
        .map(|owner| {
            fs::read(root.join(format!("{owner}.setup-host/worker.json")))
                .expect("completed evidence")
        })
        .into();
    let host = setup(root, "idp", addresses[0]);
    let listener = host.tls().listener(
        tokio::net::TcpListener::bind(addresses[0])
            .await
            .expect("delayed authority"),
    );
    let delayed_stop = CancellationToken::new();
    let delayed = tokio::spawn({
        let stop = delayed_stop.clone();
        async move {
            axum::serve(
                listener,
                axum::Router::new()
                    .fallback(|| async {
                        tokio::time::sleep(Duration::from_secs(6)).await;
                        StatusCode::SERVICE_UNAVAILABLE
                    })
                    .into_make_service(),
            )
            .with_graceful_shutdown(stop.cancelled_owned())
            .await
        }
    });
    let shutdown = CancellationToken::new();
    let mut config = storage_server::AppConfig {
        data_dir: root.join("storage").to_string_lossy().into_owned(),
        ..Default::default()
    };
    config.server.prefix = Some("/storage".into());
    let server = tokio::spawn(storage_server::serve_independent_create(
        Arc::new(config),
        setup(root, "storage", addresses[2]),
        shutdown.clone(),
    ));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(response) = http
            .get(format!("{}/databases", intent.storage))
            .send()
            .await
        {
            if response.status() == StatusCode::UNAUTHORIZED {
                break;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "completed router restored before authority responds"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let status = http
        .get(format!("{}/independent/setup/status", intent.storage))
        .send()
        .await;
    assert!(
        status.is_ok_and(|response| response.status() == StatusCode::SERVICE_UNAVAILABLE),
        "actual remote authority returns SERVICE_UNAVAILABLE after delay"
    );
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(
        !server.is_finished(),
        "delayed authority cannot cancel completed Storage listener"
    );
    delayed_stop.cancel();
    delayed.await.expect("join delayed").expect("stop delayed");
    drop(host);
    current_status(http, &intent.storage, false).await;
    assert_eq!(
        http.get(format!("{}/databases", intent.storage))
            .send()
            .await
            .expect("normal auth while unavailable")
            .status(),
        StatusCode::UNAUTHORIZED
    );
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
    let authorities = [
        tokio::spawn(idp_server::serve_independent_create(
            Arc::new(idp),
            setup(root, "idp", addresses[0]),
            shutdown.child_token(),
        )),
        tokio::spawn(device_server::serve_independent_create(
            Arc::new(device),
            setup(root, "device", addresses[1]),
            shutdown.child_token(),
        )),
    ];
    wait(http, intent, true).await;
    assert!(
        !server.is_finished(),
        "same Storage host recovers without restart"
    );
    shutdown.cancel();
    server.await.expect("join Storage").expect("stop Storage");
    for server in authorities {
        server
            .await
            .expect("join recovered")
            .expect("stop recovered");
    }
    for (owner, expected) in ["idp", "device", "storage"].into_iter().zip(evidence) {
        assert_eq!(
            fs::read(root.join(format!("{owner}.setup-host/worker.json")))
                .expect("retained evidence"),
            expected,
            "no reseeding during authority recovery"
        );
    }
}
