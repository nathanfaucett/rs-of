use std::{
    fs,
    net::SocketAddr,
    path::Path,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{Path as HttpPath, State},
    http::StatusCode,
    routing::{get, post},
};
use db::Database;
use device_service::{
    HostedControlPlane,
    replica::{DbDeviceRepo, DbSelectionPolicyRepo, SelectionPolicy},
};
use idp_model::{
    contract::{IdentityAction, IntrospectionResponse},
    model::Id,
};
use model::contract::{PrincipalType, StandardClaims, TokenType, TokenUse};

const CHILD: &str = "device_authority_tests::remote::remote_device_child";
const SCOPES: &str =
    "management.device.list management.replication.read management.replication.admit";

fn remote(endpoint: &str) -> db::RemoteDatabaseConfig {
    db::RemoteDatabaseConfig {
        endpoint: endpoint.into(),
        ca_certificate: None,
        client_certificate: None,
        client_key: None,
        deadline_ms: 2000,
    }
}
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn address() -> SocketAddr {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("reserve port")
        .local_addr()
        .expect("address")
}
fn start(
    root: &Path,
    mode: &str,
    sql: SocketAddr,
    device: SocketAddr,
    storage: SocketAddr,
) -> Process {
    let log = fs::File::create(root.join(format!("{mode}.log"))).expect("child log");
    Process(
        Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", CHILD, "--nocapture"])
            .env("OF_DEVICE_CHILD", mode)
            .env("OF_DEVICE_ROOT", root)
            .env("OF_DEVICE_SQL", sql.to_string())
            .env("OF_DEVICE_HTTP", device.to_string())
            .env("OF_STORAGE_HTTP", storage.to_string())
            .env("RUST_MIN_STACK", "67108864")
            .stdout(Stdio::from(log.try_clone().expect("clone log")))
            .stderr(Stdio::from(log))
            .spawn()
            .expect("start process"),
    )
}
async fn ready(process: &mut Process, root: &Path, mode: &str) {
    tokio::time::timeout(Duration::from_secs(60), async {
        while !root.join(format!("{mode}.ready")).exists() {
            if let Some(status) = process.0.try_wait().expect("child status") {
                panic!(
                    "{mode} exited {status}: {}",
                    fs::read_to_string(root.join(format!("{mode}.log"))).expect("child output")
                );
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("bounded child startup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_device_process_admission_and_consumer_reset_preserve_shared_rows() {
    let root = std::env::temp_dir().join(format!("remote-device-{}", Id::now_v7()));
    fs::create_dir(&root).expect("fixture root");
    let (sql, device, storage) = (address(), address(), address());
    let mut owner = start(&root, "sql", sql, device, storage);
    ready(&mut owner, &root, "sql").await;
    let mut authority = start(&root, "device", sql, device, storage);
    ready(&mut authority, &root, "device").await;
    let mut consumer = start(&root, "storage", sql, device, storage);
    ready(&mut consumer, &root, "storage").await;
    let fixture: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("fixture.json")).expect("fixture"))
            .expect("public fixture");
    let endpoint = fixture["endpoint"].as_str().expect("endpoint");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("bounded HTTP");
    let probe = format!("http://{storage}/admission/{endpoint}");
    assert_eq!(
        client
            .get(&probe)
            .send()
            .await
            .expect("trusted admission")
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        client
            .get(format!(
                "http://{storage}/admission/{}",
                iroh::SecretKey::generate().public()
            ))
            .send()
            .await
            .expect("unknown admission")
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .get(format!(
                "http://{device}/device/replication/devices/{endpoint}/selections"
            ))
            .send()
            .await
            .expect("unauthenticated request")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    drop(authority);
    fs::remove_file(root.join("device.ready")).expect("reset readiness");
    let mut reset = start(&root, "reset", sql, device, storage);
    ready(&mut reset, &root, "reset").await;
    authority = start(&root, "device", sql, device, storage);
    ready(&mut authority, &root, "device").await;
    assert_eq!(
        client
            .get(&probe)
            .send()
            .await
            .expect("rows survive consumer reset")
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(!root.join("consumer/devices.redb").exists());
    assert_eq!(
        client
            .delete(format!(
                "http://{device}/device/devices/{}",
                fixture["device"].as_str().expect("Device ID")
            ))
            .bearer_auth("administrator")
            .send()
            .await
            .expect("revoke via authority")
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        client
            .get(&probe)
            .send()
            .await
            .expect("revoked admission")
            .status(),
        StatusCode::FORBIDDEN
    );
    let peer = fixture["peer"].as_str().expect("peer endpoint");
    let peer_probe = format!("http://{storage}/admission/{peer}");
    assert_eq!(
        client
            .get(&peer_probe)
            .send()
            .await
            .expect("remaining trusted peer")
            .status(),
        StatusCode::NO_CONTENT
    );
    drop(owner);
    assert_eq!(
        client
            .get(&peer_probe)
            .send()
            .await
            .expect("SQL outage admission")
            .status(),
        StatusCode::FORBIDDEN
    );
    drop(authority);
    assert_eq!(
        client
            .get(&peer_probe)
            .send()
            .await
            .expect("Device outage admission")
            .status(),
        StatusCode::FORBIDDEN
    );
    drop(consumer);
    drop(reset);
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_device_child() {
    let Ok(mode) = std::env::var("OF_DEVICE_CHILD") else {
        return;
    };
    let root = std::path::PathBuf::from(std::env::var("OF_DEVICE_ROOT").expect("root"));
    let sql: SocketAddr = std::env::var("OF_DEVICE_SQL")
        .expect("SQL address")
        .parse()
        .expect("address");
    let device: SocketAddr = std::env::var("OF_DEVICE_HTTP")
        .expect("Device address")
        .parse()
        .expect("address");
    let storage: SocketAddr = std::env::var("OF_STORAGE_HTTP")
        .expect("Storage address")
        .parse()
        .expect("address");
    if mode == "sql" {
        let database = Database::open(root.join("devices.redb")).expect("domain owner");
        device_service::replica::up(&database)
            .await
            .expect("owner migration");
        let repo = DbDeviceRepo::new(Arc::new(database.clone()).client());
        let owner = Id::now_v7();
        let mut endpoints = Vec::new();
        let mut ids = Vec::new();
        for name in ["replica", "peer"] {
            let endpoint = iroh::SecretKey::generate().public().to_string();
            let id = Id::now_v7();
            repo.enroll_approved_exact(id, owner, name, &endpoint, 1)
                .await
                .expect("trusted fixture");
            DbSelectionPolicyRepo::new(Arc::new(database.clone()).client())
                .set_prevalidated(SelectionPolicy {
                    device_id: id,
                    owner_subject: owner.to_string(),
                    application_id: None,
                    selected_kind: None,
                    selected_id: None,
                    admin_allowed: true,
                })
                .await
                .expect("trusted policy");
            endpoints.push(endpoint);
            ids.push(id);
        }
        fs::write(root.join("fixture.json"), serde_json::to_vec(&serde_json::json!({"owner":owner, "device":ids[0], "endpoint":endpoints[0], "peer":endpoints[1]})).expect("fixture JSON")).expect("public fixture");
        fs::write(root.join("sql.ready"), "ready").expect("SQL ready");
        database
            .serve_tcp_with_mtls(sql, None, None, std::future::pending::<()>())
            .await
            .expect("SQL listener");
        return;
    }
    let config = device_server::AppConfig {
        data_dir: root.join("consumer").to_string_lossy().into_owned(),
        device_database: Some(remote(&format!("http://{sql}"))),
        ..Default::default()
    };
    if mode == "reset" {
        device_server::reset_independent(&config, false)
            .await
            .expect("remote consumer reset without deleting shared state");
        fs::write(root.join("reset.ready"), "ready").expect("reset ready");
        return;
    }
    let router = if mode == "storage" {
        let client = storage_server::DeviceClient::new(
            &format!("http://{device}/device"),
            &format!("http://{device}/idp"),
            "storage-client",
            "storage-secret",
            "https://installation.example",
            "storage",
        )
        .expect("authenticated Device client");
        let admission = storage_server::LocalReplicationAuthorizer::from_device_client(client);
        async fn probe(
            State(admission): State<storage_server::LocalReplicationAuthorizer>,
            HttpPath(endpoint): HttpPath<String>,
        ) -> StatusCode {
            match endpoint.parse() {
                Ok(endpoint) => match admission.local_owner(endpoint).await {
                    Ok(_) => StatusCode::NO_CONTENT,
                    Err(error) => {
                        eprintln!("admission denied: {error}");
                        StatusCode::FORBIDDEN
                    }
                },
                Err(_) => StatusCode::FORBIDDEN,
            }
        }
        Router::new()
            .route("/admission/{endpoint}", get(probe))
            .with_state(admission)
    } else {
        let fixture: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("fixture.json")).expect("fixture"))
                .expect("fixture JSON");
        let owner: Id = fixture["owner"]
            .as_str()
            .expect("owner")
            .parse()
            .expect("owner ID");
        let identity = Database::in_memory();
        idp_model::replica::up(&identity)
            .await
            .expect("identity schema");
        management_service::ManagementService::new(
            management_service::replica::DbPermissionRepo::new(identity.client()),
            management_service::replica::DbRoleRepo::new(identity.client()),
        )
        .provision_initial_administrator(
            owner,
            Id::now_v7(),
            &[
                IdentityAction::DevicePairingRead,
                IdentityAction::DevicePairingUpdate,
                IdentityAction::DevicesRevoke,
            ]
            .map(|action| (Id::now_v7(), action)),
        )
        .await
        .expect("administrator permission");
        let plane = HostedControlPlane::new_with_issuer(
            "https://node.example",
            "https://installation.example",
        )
        .expect("control plane")
        .with_local_idp(
            move |token| async move {
                let human = token == "administrator";
                if !human && token != "storage-token" {
                    return Err("invalid bearer".into());
                }
                Ok(IntrospectionResponse {
                    claims: StandardClaims {
                        r#type: TokenType::Bearer,
                        r#use: TokenUse::Access,
                        exp: i64::MAX,
                        iat: 0,
                        nbf: 0,
                        iss: "https://installation.example".into(),
                        aud: if human {
                            management_service::MANAGEMENT_APPLICATION_URI.into()
                        } else {
                            "storage".into()
                        },
                        client_id: if human {
                            "human".into()
                        } else {
                            "storage-client".into()
                        },
                        sub: owner.to_string(),
                        principal_type: if human {
                            PrincipalType::User
                        } else {
                            PrincipalType::Client
                        },
                        resource: None,
                        authorization_details: None,
                        scope: SCOPES.split_whitespace().map(str::to_owned).collect(),
                    },
                    application_id: owner.to_string(),
                })
            },
            move |user| async move {
                if user == owner {
                    Ok(())
                } else {
                    Err("inactive owner".into())
                }
            },
        );
        let devices = device_server::device_client(&config)
            .await
            .expect("configured remote Device startup");
        let router = device_server::build_router(
            identity.client(),
            devices,
            "https://node.example",
            "/device",
            "storage",
            Arc::new(plane),
            None,
        )
        .await
        .expect("Device router");
        async fn token(body: String) -> Result<Json<serde_json::Value>, StatusCode> {
            if !body.contains("storage-client") || !body.contains("storage-secret") {
                return Err(StatusCode::UNAUTHORIZED);
            }
            Ok(Json(
                serde_json::json!({"access_token":"storage-token", "token_type":"Bearer", "expires_in":300, "iss":"https://installation.example", "scope":SCOPES}),
            ))
        }
        router.route("/idp/oauth2/token", post(token))
    };
    let listener = tokio::net::TcpListener::bind(if mode == "storage" { storage } else { device })
        .await
        .expect("HTTP listener");
    fs::write(root.join(format!("{mode}.ready")), "ready").expect("HTTP ready");
    axum::serve(listener, router).await.expect("HTTP server");
}
