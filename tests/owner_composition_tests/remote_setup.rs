use std::{
    fs,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use device_service::{DeviceRepo, ofdb::DbDeviceRepo};
use idp_service::{PasswordConfig, ofdb::DbUserRepo, repo::UserRepo};
use of_model::idp::model::Id;
use reqwest::StatusCode;
use server_common::{IndependentCreateIntent, LocalTls};
use tokio_util::sync::CancellationToken;

use super::create::{normal_use, setup, setup_code, start, wait};

#[test]
#[ignore = "requires built service launcher and identity-database"]
fn remote_create_reopen_and_reset_preserve_owner_databases() {
    let root = std::env::temp_dir().join(format!("remote-setup-{}", Id::now_v7()));
    fs::create_dir(&root).expect("isolated root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private root");
    let mut addresses = Vec::new();
    let mut bundle = Vec::new();
    {
        let directory = root.join("source");
        fs::create_dir(&directory).expect("consumer root");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("private consumer root");
        for owner in ["idp", "device", "storage"] {
            let reservation = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve address");
            let address = reservation.local_addr().expect("address");
            let host = setup(&directory, owner, address);
            bundle.extend(fs::read(host.tls().ca_path()).expect("consumer CA"));
            addresses.push(address);
        }
    }
    fs::write(
        root.join("addresses.json"),
        serde_json::to_vec(&addresses).expect("addresses JSON"),
    )
    .expect("addresses");
    fs::write(root.join("roots.pem"), bundle).expect("isolated native roots");
    fs::create_dir(root.join("empty-roots")).expect("isolated root directory");
    fs::create_dir(root.join("commands")).expect("mock tools");
    fs::write(root.join("commands/trust"), "#!/bin/sh\nexit 0\n").expect("mock trust");
    fs::set_permissions(
        root.join("commands/trust"),
        fs::Permissions::from_mode(0o700),
    )
    .expect("executable stub");
    super::reset::prepare_trust_removal(&root);
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "owner_composition_tests::remote_setup::remote_setup_child",
            "--ignored",
            "--nocapture",
        ])
        .env("OF_REMOTE_SETUP_ROOT", &root)
        .env("SSL_CERT_FILE", root.join("roots.pem"))
        .env("SSL_CERT_DIR", root.join("empty-roots"))
        .env("PATH", root.join("commands"))
        .env("NO_PROXY", "127.0.0.1,::1,localhost")
        .env("RUST_MIN_STACK", "67108864")
        .stdin(Stdio::null())
        .spawn()
        .expect("isolated remote setup child");
    let deadline = Instant::now() + Duration::from_secs(300);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("bound setup child");
            child.wait().expect("reap setup child");
            panic!("remote setup timed out; evidence at {}", root.display());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        status.success(),
        "remote setup child failed; evidence at {}",
        root.display()
    );
    fs::remove_dir_all(root).expect("fixture cleanup");
}

fn remote(address: SocketAddr) -> db::RemoteDatabaseConfig {
    db::RemoteDatabaseConfig {
        endpoint: format!("http://{address}"),
        ca_certificate: None,
        client_certificate: None,
        client_key: None,
        deadline_ms: 20_000,
    }
}
fn intent(addresses: &[SocketAddr], issuer: &str) -> IndependentCreateIntent {
    IndependentCreateIntent {
        purpose: "create".into(),
        operation_nonce: Id::now_v7().simple().to_string(),
        administrator: "operator".into(),
        node: "Remote Node".into(),
        idp: format!("https://{}/idp", addresses[0]),
        device: format!("https://{}/device", addresses[1]),
        storage: format!("https://{}/storage", addresses[2]),
        issuer: issuer.into(),
    }
}
async fn submit(http: &reqwest::Client, root: &Path, intent: &IndependentCreateIntent) {
    for owner in ["idp", "device", "storage"] {
        let mut request = serde_json::to_value(intent).expect("intent JSON");
        if owner == "idp" && intent.purpose == "create" {
            request["password"] = serde_json::json!("protected-password");
        }
        let response = http
            .post(format!(
                "{}/setup/owner/{}",
                intent.base(owner).expect("base"),
                intent.purpose
            ))
            .header(
                "Authorization",
                format!("Setup {}", setup_code(root, owner)),
            )
            .json(&request)
            .send()
            .await
            .expect("submit setup");
        assert_eq!(
            response.status(),
            StatusCode::ACCEPTED,
            "{owner} setup admission"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "parent isolates native roots, protected provider and OS trust tools"]
async fn remote_setup_child() {
    let root = PathBuf::from(std::env::var_os("OF_REMOTE_SETUP_ROOT").expect("fixture root"));
    crate::support::timing::capture_setup_timings();
    let _provider = super::reset::Provider::install();
    let addresses: Vec<SocketAddr> =
        serde_json::from_slice(&fs::read(root.join("addresses.json")).expect("addresses"))
            .expect("address JSON");
    fs::create_dir(root.join("owners")).expect("separate domain owner roots");
    let identity = db::Database::open(root.join("owners/identity.redb")).expect("identity owner");
    of_model::ofdb::up(&identity.client())
        .await
        .expect("identity migration");
    let devices = db::Database::open(root.join("owners/devices.redb")).expect("Device owner");
    device_service::ofdb::up(&devices)
        .await
        .expect("Device migration");
    let identity_address = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("reserve identity SQL")
        .local_addr()
        .expect("address");
    let device_address = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("reserve Device SQL")
        .local_addr()
        .expect("address");
    let sql_shutdown = CancellationToken::new();
    let identity_stop = sql_shutdown.clone();
    let device_stop = sql_shutdown.clone();
    let identity_task = tokio::spawn(async move {
        identity
            .serve_tcp_with_mtls(
                identity_address,
                None,
                None,
                identity_stop.cancelled_owned(),
            )
            .await
    });
    let device_task = tokio::spawn(async move {
        devices
            .serve_tcp_with_mtls(device_address, None, None, device_stop.cancelled_owned())
            .await
    });
    let shared = (remote(identity_address), remote(device_address));
    {
        fs::write(
            root.join("source/shared-sql.json"),
            serde_json::to_vec(&shared).expect("remote config JSON"),
        )
        .expect("remote config");
    }
    for config in [&shared.0, &shared.1] {
        tokio::time::timeout(Duration::from_secs(10), async {
            while config.connect().await.is_err() {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("SQL startup");
    }
    let http = LocalTls::verification_client().expect("verified HTTPS");
    let source_root = root.join("source");
    let source_stop = CancellationToken::new();
    let source_servers = start(&source_root, &addresses[..3], &source_stop).await;
    let source = intent(&addresses[..3], &format!("https://{}/idp", addresses[0]));
    wait(&http, &source, false).await;
    submit(&http, &source_root, &source).await;
    wait(&http, &source, true).await;
    let (source_endpoint, _, _) = normal_use(&http, &source).await;
    assert!(!source_root.join("idp/idp.redb").exists());
    assert!(!source_root.join("device/devices.redb").exists());
    source_stop.cancel();
    for server in source_servers {
        server.await.expect("source task").expect("source shutdown");
    }
    let reopen_stop = CancellationToken::new();
    let reopened = start(&source_root, &addresses, &reopen_stop).await;
    wait(&http, &source, true).await;
    assert_eq!(normal_use(&http, &source).await.0, source_endpoint);
    reopen_stop.cancel();
    for server in reopened {
        server
            .await
            .expect("join reopened task")
            .expect("reopened shutdown");
    }
    let reset_config = device_server::AppConfig {
        data_dir: source_root.join("device").to_string_lossy().into_owned(),
        device_database: Some(shared.1.clone()),
        identity_database: Some(shared.0.clone()),
        ..Default::default()
    };
    let retained = source_root.join("device/shared-owner.redb");
    fs::write(&retained, "unexpected owner evidence").expect("retained owner fixture");
    assert!(
        device_server::reset_independent(&reset_config, true, || Ok(()))
            .await
            .is_err(),
        "remote reset must refuse unknown owner data even with valid setup evidence"
    );
    assert_eq!(
        fs::read_to_string(&retained).expect("retained evidence"),
        "unexpected owner evidence"
    );
    fs::remove_file(retained).expect("remove injected fixture");
    device_server::reset_independent(&reset_config, true, || Ok(()))
        .await
        .expect("consumer-only Device reset");
    let device_client = shared
        .1
        .connect()
        .await
        .expect("remote Device query client");
    assert!(
        DbDeviceRepo::new(device_client)
            .find_approved_by_public_key(&source_endpoint)
            .await
            .expect("retained remote Device")
            .is_some()
    );
    let identity_client = shared
        .0
        .connect()
        .await
        .expect("shared identity query client");
    let users = DbUserRepo::new(identity_client, PasswordConfig::default());
    let owner = users
        .find_user_by_username_or_email("operator")
        .await
        .expect("remote IdP User query")
        .expect("retained initial administrator")
        .id;
    idp_server::reset_independent(
        &idp_server::AppConfig {
            data_dir: source_root.join("idp").to_string_lossy().into_owned(),
            identity_database: Some(shared.0.clone()),
            ..Default::default()
        },
        true,
        || Ok(()),
    )
    .await
    .expect("remote Create IdP consumer reset");
    assert!(
        users
            .find_user_by_id(owner)
            .await
            .expect("remote IdP owner still serves")
            .is_some(),
        "consumer reset must preserve the remote User catalog"
    );
    sql_shutdown.cancel();
    identity_task
        .await
        .expect("identity task")
        .expect("identity SQL shutdown");
    device_task
        .await
        .expect("Device task")
        .expect("Device SQL shutdown");
}
