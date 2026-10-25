use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};

use crate::support::identity::{
    PermissionFixture, expect_status, seed_idp_with_config, seed_permissions_on, user_token,
};

const CHILD_TEST: &str =
    "permission_http_tests::remote_identity_process_tests::remote_identity_child";

fn remote(endpoint: &str) -> db::RemoteDatabaseConfig {
    db::RemoteDatabaseConfig {
        endpoint: endpoint.into(),
        ca_certificate: None,
        client_certificate: None,
        client_key: None,
        deadline_ms: 20_000,
    }
}

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start(
    root: &Path,
    mode: &str,
    sql: SocketAddr,
    idp: SocketAddr,
    management: SocketAddr,
) -> Process {
    let log = fs::File::create(root.join(format!("{mode}.log"))).expect("child log");
    Process(
        Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", CHILD_TEST, "--nocapture"])
            .env("OF_REMOTE_IDENTITY_CHILD", mode)
            .env("OF_REMOTE_IDENTITY_ROOT", root)
            .env("OF_REMOTE_IDENTITY_SQL", sql.to_string())
            .env("OF_REMOTE_IDENTITY_IDP", idp.to_string())
            .env("OF_REMOTE_IDENTITY_MANAGEMENT", management.to_string())
            .env("RUST_MIN_STACK", "16777216")
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    root.join("os-tools").display(),
                    std::env::var("PATH").expect("PATH")
                ),
            )
            .stdout(Stdio::from(log.try_clone().expect("clone log")))
            .stderr(Stdio::from(log))
            .spawn()
            .expect("start independent process"),
    )
}

fn address() -> SocketAddr {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("reserve address")
        .local_addr()
        .expect("address")
}

async fn ready(process: &mut Process, root: &Path, mode: &str) {
    tokio::time::timeout(Duration::from_secs(90), async {
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
    .unwrap_or_else(|_| panic!("{mode} startup timed out; logs at {}", root.display()));
}

#[test]
fn independent_processes_share_remote_identity_and_consumer_reset_preserves_rows() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_stack_size(16 * 1024 * 1024)
                .enable_all()
                .build()
                .expect("acceptance runtime")
                .block_on(acceptance());
        })
        .expect("acceptance thread")
        .join()
        .expect("remote identity acceptance");
}

async fn acceptance() {
    let root = std::env::temp_dir().join(format!(
        "remote-identity-{}",
        idp_model::model::Id::now_v7()
    ));
    fs::create_dir(&root).expect("fixture directory");
    let tools = root.join("os-tools");
    fs::create_dir(&tools).expect("isolated host tools");
    for name in ["trust", "pkexec"] {
        use std::os::unix::fs::PermissionsExt;
        let path = tools.join(name);
        fs::write(&path, "#!/bin/sh\nexit 0\n")
            .expect("mock OS trust tools; acceptance must not change host trust");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .expect("executable host-tool stub");
    }
    let (sql, idp, management) = (address(), address(), address());
    let mut owner = start(&root, "sql", sql, idp, management);
    ready(&mut owner, &root, "sql").await;
    let mut identity = start(&root, "idp", sql, idp, management);
    ready(&mut identity, &root, "idp").await;
    let mut policy = start(&root, "management", sql, idp, management);
    ready(&mut policy, &root, "management").await;
    let fixture: PermissionFixture = serde_json::from_slice(
        &fs::read(root.join("fixture.json")).expect("public fixture identities"),
    )
    .expect("fixture");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("bounded HTTP client");
    let idp_base = format!("http://{idp}");
    let management_base = format!("http://{management}");
    let actor = user_token(
        &client,
        &idp_base,
        "administrator",
        management_service::MANAGEMENT_APPLICATION_URI,
    )
    .await;
    let subject = user_token(&client, &idp_base, "unassigned", "idp-services").await;
    let registration = format!("{idp_base}/idp/oauth2/register/human-client");
    expect_status(
        client
            .get(&registration)
            .bearer_auth(&subject)
            .send()
            .await
            .expect("unassigned request"),
        http::StatusCode::FORBIDDEN,
    )
    .await;
    let application = format!(
        "{management_base}/management/applications/{}",
        fixture.application_id
    );
    let permissions = expect_status(
        client
            .get(format!("{application}/permissions"))
            .bearer_auth(&actor)
            .send()
            .await
            .expect("read permissions"),
        http::StatusCode::OK,
    )
    .await;
    let permission = permissions
        .as_array()
        .expect("permissions")
        .iter()
        .find(|row| row["name"] == "idp.clients.read")
        .expect("read permission")["id"]
        .as_str()
        .expect("permission ID");
    let role = expect_status(
        client
            .post(format!("{application}/roles"))
            .bearer_auth(&actor)
            .json(&serde_json::json!({"name": "remote identity reader"}))
            .send()
            .await
            .expect("create role via Management process"),
        http::StatusCode::OK,
    )
    .await;
    let role = role["id"].as_str().expect("role ID");
    expect_status(
        client
            .post(format!(
                "{application}/roles/{role}/permissions/{permission}"
            ))
            .bearer_auth(&actor)
            .send()
            .await
            .expect("assign permission"),
        http::StatusCode::OK,
    )
    .await;
    let assignment = format!(
        "{application}/users/{}/roles/{role}",
        fixture.unassigned_user
    );
    expect_status(
        client
            .post(&assignment)
            .bearer_auth(&actor)
            .send()
            .await
            .expect("assign role"),
        http::StatusCode::OK,
    )
    .await;
    expect_status(
        client
            .get(&registration)
            .bearer_auth(&subject)
            .send()
            .await
            .expect("IdP sees Management write"),
        http::StatusCode::OK,
    )
    .await;
    drop(policy);
    fs::remove_file(root.join("management.ready")).expect("remove stale readiness");
    let mut reset = start(&root, "reset", sql, idp, management);
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(status) = reset.0.try_wait().expect("reset status") {
                assert!(
                    status.success(),
                    "reset output: {}",
                    fs::read_to_string(root.join("reset.log")).expect("reset log")
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("bounded consumer reset");
    expect_status(
        client
            .get(&registration)
            .bearer_auth(&subject)
            .send()
            .await
            .expect("reset preserves live IdP authorization"),
        http::StatusCode::OK,
    )
    .await;
    let mut policy = start(&root, "management", sql, idp, management);
    ready(&mut policy, &root, "management").await;
    expect_status(
        client
            .delete(&assignment)
            .bearer_auth(&actor)
            .send()
            .await
            .expect("revoke after restart"),
        http::StatusCode::OK,
    )
    .await;
    expect_status(
        client
            .get(&registration)
            .bearer_auth(&subject)
            .send()
            .await
            .expect("IdP sees revocation"),
        http::StatusCode::FORBIDDEN,
    )
    .await;
    assert!(
        !root.join("idp/idp.redb").exists(),
        "remote IdP must not create a private identity file"
    );
    drop(owner);
    let response = client
        .get(&registration)
        .bearer_auth(&subject)
        .send()
        .await
        .expect("owner outage denies request");
    assert!(
        !response.status().is_success(),
        "SQL outage must fail closed"
    );
    drop(policy);
    drop(identity);
    fs::remove_dir_all(root).expect("remove fixture after all processes exit");
}

#[test]
fn remote_identity_child() {
    let Ok(mode) = std::env::var("OF_REMOTE_IDENTITY_CHILD") else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("OF_REMOTE_IDENTITY_ROOT").expect("root"));
    let sql: SocketAddr = std::env::var("OF_REMOTE_IDENTITY_SQL")
        .expect("SQL address")
        .parse()
        .expect("address");
    let idp: SocketAddr = std::env::var("OF_REMOTE_IDENTITY_IDP")
        .expect("IdP address")
        .parse()
        .expect("address");
    let management: SocketAddr = std::env::var("OF_REMOTE_IDENTITY_MANAGEMENT")
        .expect("Management address")
        .parse()
        .expect("address");
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(16 * 1024 * 1024)
        .enable_all()
        .build()
        .expect("child runtime")
        .block_on(async {
            let endpoint = format!("http://{sql}");
            let issuer = format!(
                "https://{}.example",
                root.file_name().expect("fixture name").to_string_lossy()
            );
            if mode == "sql" {
                let database = db::Database::open(root.join("identity.redb"))
                    .expect("identity database owner");
                idp_model::replica::up(&database)
                    .await
                    .expect("owner migrations");
                fs::write(root.join("sql.ready"), "ready").expect("owner ready");
                database
                    .serve_tcp(sql, std::future::pending())
                    .await
                    .expect("SQL owner serving");
            } else if mode == "idp" {
                let mut config = idp_server::AppConfig {
                    data_dir: root.join("idp").to_string_lossy().into_owned(),
                    identity_database: Some(remote(&endpoint)),
                    service_audience: Some("idp-services".into()),
                    ..Default::default()
                };
                config.oauth2.issuer = issuer;
                config.server.prefix = Some("/idp".into());
                let transport = iroh::SecretKey::generate().public().to_string();
                let mut fixture = seed_idp_with_config(
                    &root,
                    &config,
                    &transport,
                    &iroh::SecretKey::generate().public().to_string(),
                )
                .await;
                let client = idp_server::identity_client(&config)
                    .await
                    .expect("configured remote IdP startup");
                seed_permissions_on(client.clone(), &mut fixture).await;
                config.oauth2.local_signer = Some(fixture.local_signer.clone());
                let runtime = idp_server::build_oauth_runtime(&config, client)
                    .await
                    .expect("remote OAuth runtime with IdP-local private signer");
                fs::write(
                    root.join("fixture.json"),
                    serde_json::to_vec(&fixture).expect("public fixture"),
                )
                .expect("fixture state");
                let listener = tokio::net::TcpListener::bind(idp)
                    .await
                    .expect("IdP listener");
                fs::write(root.join("idp.ready"), "ready").expect("IdP ready");
                axum::serve(listener, runtime.router())
                    .await
                    .expect("serve IdP process");
            } else if mode == "management" {
                let config = management_server::AppConfig {
                    data_dir: root
                        .join("management-consumer")
                        .to_string_lossy()
                        .into_owned(),
                    identity_database: Some(remote(&endpoint)),
                    ..Default::default()
                };
                assert!(!Path::new(&config.data_dir).exists());
                let client = management_server::identity_client(&config)
                    .await
                    .expect("configured remote Management startup without migrations");
                let control = device_service::HostedControlPlane::new_with_services(
                    &format!("http://{idp}/idp"),
                    &format!("http://{idp}/unused-storage"),
                    &issuer,
                )
                .expect("IdP relationship")
                .with_idp_service_client("management-idp", "management-idp-secret", "idp-services")
                .expect("introspection client");
                let router = management_server::build_router(
                    client,
                    &format!("http://{management}"),
                    "/management",
                    Arc::new(control),
                )
                .await
                .expect("remote Management router");
                let listener = tokio::net::TcpListener::bind(management)
                    .await
                    .expect("Management listener");
                fs::write(root.join("management.ready"), "ready").expect("Management ready");
                axum::serve(listener, router)
                    .await
                    .expect("serve Management process");
            } else if mode == "reset" {
                let data = root.join("management-consumer");

                let config = management_server::AppConfig {
                    data_dir: data.to_string_lossy().into_owned(),
                    identity_database: Some(remote(&endpoint)),
                    ..Default::default()
                };
                management_server::reset_independent(&config, true)
                    .await
                    .expect("reset only consumer local state");
                assert!(!data.exists());
                assert!(!data.with_extension("setup-host").exists());
            } else {
                panic!("unknown child mode");
            }
        });
}
