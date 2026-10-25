use std::{
    fs, io,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

use api::{IndependentCreateIntent, LocalTls};
use device_service::DeviceIdentity;
use device_service::HostedControlPlane;
use idp_model::{
    contract::{ClientProfile, ClientType, GrantType, IdentityAction, TokenEndpointAuthMethod},
    model::Id,
};
use idp_service::{
    oauth2::NodeSignerBinding,
    setup_credentials::{SetupRelationship, create_setup_secret, load_setup_secret},
};
use reqwest::{Client, StatusCode};
use tokio_util::sync::CancellationToken;

use crate::owner_composition_tests::create::{setup, setup_code, start, wait, wait_public};

#[test]
fn production_independent_join_and_source_offline_reopen() {
    run_production_join(false);
}

#[test]
fn production_independent_join_legacy_record_reset() {
    run_production_join(true);
}

fn run_production_join(legacy_reset: bool) {
    let root = std::env::temp_dir().join(format!("independent-join-{}", Id::now_v7()));
    fs::create_dir(&root).expect("isolated fixture");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private fixture");
    let mut addresses = Vec::new();
    let mut bundle = Vec::new();
    for side in ["source", "target"] {
        let directory = root.join(side);
        fs::create_dir(&directory).expect("separate deployment roots");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("private deployment");
        for owner in ["idp", "device", "storage"] {
            let reservation =
                std::net::TcpListener::bind("127.0.0.1:0").expect("reserve owner address");
            let address = reservation.local_addr().expect("owner address");
            if side == "source" {
                let tls = LocalTls::create(&SourceTls {
                    directory: directory.join(format!("{owner}-tls")),
                    address,
                })
                .expect("existing source TLS prerequisite");
                bundle.extend(fs::read(tls.ca_path()).expect("source native CA"));
            } else {
                let host = setup(&directory, owner, address);
                bundle.extend(fs::read(host.tls().ca_path()).expect("target native CA"));
            }
            addresses.push(address);
            drop(reservation);
        }
    }
    fs::write(
        root.join("addresses.json"),
        serde_json::to_vec(&addresses).expect("addresses"),
    )
    .expect("fixture metadata");
    fs::write(root.join("roots.pem"), bundle).expect("isolated native roots");
    fs::create_dir(root.join("empty-roots")).expect("native root directory");
    fs::create_dir(root.join("commands")).expect("injected command directory");
    fs::write(root.join("commands/trust"), "#!/bin/sh\nexit 0\n").expect("mock trust command");
    fs::set_permissions(
        root.join("commands/trust"),
        fs::Permissions::from_mode(0o700),
    )
    .expect("executable mock");
    crate::owner_composition_tests::reset::prepare_trust_removal(&root);
    let mut child = Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "owner_composition_tests::join::independent_join_child",
            "--ignored",
            "--nocapture",
        ])
        .env("OF_INDEPENDENT_JOIN_ROOT", &root)
        .env(
            "OF_INDEPENDENT_LEGACY_RESET",
            if legacy_reset { "1" } else { "0" },
        )
        .env("SSL_CERT_FILE", root.join("roots.pem"))
        .env("SSL_CERT_DIR", root.join("empty-roots"))
        .env("PATH", root.join("commands"))
        .env("NO_PROXY", "127.0.0.1,::1,localhost")
        .stdin(Stdio::null())
        .spawn()
        .expect("isolated existing-source Join child");
    let deadline = Instant::now() + Duration::from_secs(180);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("bound fixture");
            child.wait().expect("reap child");
            panic!("independent Join exceeded 180-second bound");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    fs::remove_dir_all(root).expect("isolated cleanup");
    assert!(status.success(), "production independent Join child");
}

struct SourceTls {
    directory: PathBuf,
    address: SocketAddr,
}
impl api::LocalTlsConfig for SourceTls {
    fn tls_directory(&self) -> PathBuf {
        self.directory.clone()
    }
    fn tls_address(&self) -> SocketAddr {
        self.address
    }
}

struct ExistingSource {
    idp: String,
    device: String,
    issuer: String,
    login: Id,
    identity: Arc<DeviceIdentity>,
    idp_runtime: idp_server::IdpRuntime,
    management_runtime: device_server::IndependentDeviceRuntime,
    storage_runtime: storage_server::StorageRuntime,
    tls: Vec<LocalTls>,
}
impl ExistingSource {
    async fn provision(root: &Path, addresses: &[SocketAddr]) -> Self {
        let tls = ["idp", "device", "storage"]
            .iter()
            .zip(addresses)
            .map(|(owner, address)| {
                LocalTls::open(&SourceTls {
                    directory: root.join(format!("{owner}-tls")),
                    address: *address,
                })
                .expect("existing source TLS, no setup worker admission")
            })
            .collect();
        let idp = format!("https://{}/idp", addresses[0]);
        let device = format!("https://{}/device", addresses[1]);
        let storage = format!("https://{}/storage", addresses[2]);
        let issuer = format!("https://existing-{}.installation.invalid/idp", Id::now_v7());
        let secret = iroh::SecretKey::generate();
        let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
            .secret_key(secret.clone())
            .bind()
            .await
            .expect("retained actual source endpoint");
        let identity =
            Arc::new(DeviceIdentity::new(endpoint, secret).expect("source endpoint possession"));
        let user = Id::now_v7();
        let login = Id::now_v7();
        let clients = [Id::now_v7(), Id::now_v7(), Id::now_v7(), Id::now_v7()];
        let binding = NodeSignerBinding {
            member_id: Id::now_v7(),
            key_id: Id::now_v7(),
            endpoint_id: identity.endpoint_id().to_string(),
        };
        let mut config = idp_server::AppConfig {
            data_dir: root.join("idp").to_string_lossy().into_owned(),
            api_public_uri: idp.clone(),
            service_audience: Some(idp.clone()),
            key_namespace: "existing-independent-source".into(),
            ..Default::default()
        };
        config.server.prefix = Some("/idp".into());
        config.oauth2.issuer = issuer.clone();
        config.oauth2.local_signer = Some(binding.clone());
        config.management_data_dir = None;
        config.control_plane_uri = Some(device.clone());
        let service = format!("existing-source:{}", root.display());
        for client in clients {
            create_setup_secret(&service, &client.to_string())
                .expect("explicit fixture provider preparation");
        }
        let owner = idp_server::OwnerProvisioner::open(&config)
            .await
            .expect("existing source IdP owner");
        let mut inventory = clients.to_vec();
        inventory.push(login);
        owner
            .prepare_fresh_setup_secret_inventory(user, &inventory)
            .await
            .expect("source secure inventory");
        owner
            .prepare_setup_client_slots(&inventory)
            .await
            .expect("source Client inventory");
        owner
            .ensure_initial_user(
                user,
                Id::now_v7(),
                "source-operator",
                "source-protected-password",
            )
            .await
            .expect("existing source administrator");
        owner
            .ensure_initial_user_root(user)
            .await
            .expect("existing User root");
        owner
            .ensure_initial_node(&binding, user, "Existing source")
            .await
            .expect("existing source signer");
        for (relationship, client) in [
            SetupRelationship::IdpManagement,
            SetupRelationship::ManagementIdp,
            SetupRelationship::StorageIdp,
            SetupRelationship::StorageManagement,
        ]
        .into_iter()
        .zip(clients)
        {
            let mut registration = relationship.registration(client, &idp, &device, &storage);
            registration.client_secret = Some(
                load_setup_secret(&service, &client.to_string())
                    .expect("fixture-only relationship"),
            );
            owner
                .ensure_setup_infrastructure_client(registration)
                .await
                .expect("existing authenticated relationship");
        }
        let mut registration =
            SetupRelationship::ManagementIdp.registration(login, &idp, &device, &storage);
        registration.client_type = ClientType::Public;
        registration.profile = ClientProfile::Native;
        registration.token_endpoint_auth_method = TokenEndpointAuthMethod::None;
        registration.client_name = "Existing source public login".into();
        registration.client_uri = None;
        registration.allowed_grant_types = vec![
            GrantType::Password,
            GrantType::RefreshToken,
            GrantType::TokenExchange,
        ];
        registration.allowed_scopes = vec!["openid".into(), "profile".into(), "email".into()];
        registration.allowed_audiences = vec![
            idp.clone(),
            format!("https://{}/idp", addresses[3]),
            "idp-management".into(),
            "storage:databases".into(),
            "storage:filesystems".into(),
        ];
        owner
            .ensure_setup_public_login_client(registration)
            .await
            .expect("existing source login");
        let mut management_config = device_server::AppConfig {
            data_dir: root.join("device").to_string_lossy().into_owned(),
            ..Default::default()
        };
        management_config.server.prefix = Some("/device".into());
        management_config.expected_issuer = issuer.clone();
        management_config.idp_api_base = idp.clone();
        management_config.api_public_uri = device.clone();
        management_config.storage_api_base = storage.clone();
        let device_engine = device_server::open_device_database(&management_config)
            .await
            .expect("existing source Management owner");
        let permissions = [
            IdentityAction::DevicePairingRead,
            IdentityAction::DevicePairingUpdate,
            IdentityAction::ReplicaSignersEnroll,
            IdentityAction::IdpSecuritySync,
            IdentityAction::ManagementSecuritySync,
            IdentityAction::DevicesRead,
            IdentityAction::ApplicationsRead,
        ]
        .map(|action| (Id::now_v7(), action));
        management_server::provision_initial_administrator(
            owner.node_join_database().expect("identity owner"),
            user,
            Id::now_v7(),
            &permissions,
        )
        .await
        .expect("explicit existing source grants");
        device_server::provision_initial_device(
            device_engine.clone().client(),
            Id::now_v7(),
            user,
            &identity.endpoint_id().to_string(),
            "Existing source",
        )
        .await
        .expect("actual source Management Device, distinct from signer");
        let plane = HostedControlPlane::new_with_services(&idp, &storage, &issuer)
            .expect("independent source services")
            .with_idp_service_client(
                clients[1].to_string(),
                load_setup_secret(&service, &clients[1].to_string())
                    .expect("source identity credential"),
                &idp,
            )
            .expect("source identity relationship")
            .with_permission_evaluator(&clients[0].to_string())
            .expect("source evaluator")
            .with_device_identity(identity.clone());
        let management_runtime = device_server::build_independent_device(
            &management_config,
            device_engine.client(),
            owner.node_join_database().expect("identity owner").client(),
            Some(device_engine),
            Arc::new(plane),
            identity.clone(),
            CancellationToken::new(),
        )
        .await
        .expect("existing source Management serving");
        let idp_runtime = idp_server::build_independent_idp(
            &config,
            owner
                .node_join_database()
                .expect("local identity owner")
                .client(),
            Some(owner.node_join_database().expect("local identity owner")),
            CancellationToken::new(),
        )
        .await
        .expect("existing independent source OAuth");
        let idp_client = storage_server::IdpClient::new(
            &idp,
            &clients[2].to_string(),
            &load_setup_secret(&service, &clients[2].to_string()).expect("source Storage identity"),
            &issuer,
            &idp,
        )
        .expect("source Storage IdP client");
        let management_client = storage_server::DeviceClient::new(
            &device,
            &idp,
            &clients[3].to_string(),
            &load_setup_secret(&service, &clients[3].to_string()).expect("source Storage policy"),
            &issuer,
            &storage,
        )
        .expect("source Storage Management client");
        let storage_runtime = storage_server::build_http_runtime(
            storage_server::RouterState::new(&storage)
                .with_idp_client(idp_client)
                .with_device_client(management_client),
            &root.join("storage"),
            "/storage",
            identity.endpoint_id(),
            CancellationToken::new(),
        )
        .expect("existing source Storage serving");
        Self {
            idp,
            device,
            issuer,
            login,
            identity,
            idp_runtime,
            management_runtime,
            storage_runtime,
            tls,
        }
    }
    async fn serve(
        &self,
        addresses: &[SocketAddr],
        cancellation: &CancellationToken,
    ) -> Vec<tokio::task::JoinHandle<io::Result<()>>> {
        let mut tasks = Vec::new();
        for ((tls, address), router) in self.tls.iter().zip(addresses).zip([
            self.idp_runtime.router(),
            self.management_runtime.router(),
            self.storage_runtime.router(),
        ]) {
            let listener = tokio::net::TcpListener::bind(address)
                .await
                .expect("separate existing source HTTPS listener");
            let listener = tls.listener(listener);
            let cancellation = cancellation.child_token();
            tasks.push(tokio::spawn(async move {
                axum::serve(listener, router)
                    .with_graceful_shutdown(cancellation.cancelled_owned())
                    .await
            }));
        }
        tasks
    }
}
async fn source_tokens(http: &Client, source: &ExistingSource) -> (String, String) {
    let login = source.login.to_string();
    let response = http
        .post(format!("{}/oauth2/token", source.idp))
        .form(&[
            ("grant_type", "password"),
            ("client_id", login.as_str()),
            ("username", "source-operator"),
            ("password", "source-protected-password"),
            ("scope", "openid"),
            ("resource", "idp-management"),
        ])
        .send()
        .await
        .expect("ordinary existing source login");
    assert_eq!(response.status(), StatusCode::OK);
    let token: model::contract::TokenResponse = response.json().await.expect("ordinary User token");
    let device = http
        .post(format!("{}/oauth2/token", source.idp))
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            ("client_id", login.as_str()),
            ("subject_token", token.access_token.0.as_str()),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("resource", "idp-management"),
        ])
        .send()
        .await
        .expect("ordinary Management audience exchange");
    assert_eq!(device.status(), StatusCode::OK);
    let device: model::contract::TokenResponse =
        device.json().await.expect("Management User token");
    let storage = http
        .post(format!("{}/oauth2/token", source.idp))
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            ("client_id", login.as_str()),
            ("subject_token", device.access_token.0.as_str()),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("resource", "storage:databases"),
            (
                "authorization_details",
                r#"[{"type":"storage","actions":["read","write"]}]"#,
            ),
        ])
        .send()
        .await
        .expect("ordinary Storage exchange");
    assert_eq!(storage.status(), StatusCode::OK);
    let storage: model::contract::TokenResponse = storage.json().await.expect("Storage User token");
    (device.access_token.0, storage.access_token.0)
}
async fn pending_offer(http: &Client, source: &ExistingSource, bearer: &str) -> serde_json::Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let response = http
            .get(format!("{}/independent/node-joins/pending", source.device))
            .bearer_auth(bearer)
            .send()
            .await
            .expect("ordinary authorized source pending offers");
        assert_eq!(response.status(), StatusCode::OK);
        let offers: serde_json::Value = response.json().await.expect("public pending offers");
        if let Some(offer) = offers.as_array().expect("pending list").first() {
            return offer.clone();
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded exact offer"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
async fn use_joined(
    http: &Client,
    intent: &IndependentCreateIntent,
    device: &str,
    storage: &str,
) -> serde_json::Value {
    assert_eq!(
        http.get(format!("{}/devices/trusted", intent.device))
            .send()
            .await
            .expect("missing User evidence")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = http
        .get(format!("{}/devices/trusted", intent.device))
        .bearer_auth(device)
        .send()
        .await
        .expect("normal joined Management");
    assert_eq!(response.status(), StatusCode::OK);
    let devices: serde_json::Value = response.json().await.expect("joined Devices");
    let response = http
        .post(format!("{}/oauth2/token", intent.idp))
        .form(&[
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:token-exchange",
            ),
            ("subject_token", device),
            (
                "subject_token_type",
                "urn:ietf:params:oauth:token-type:access_token",
            ),
            ("resource", intent.idp.as_str()),
        ])
        .send()
        .await
        .expect("joined IdP audience exchange");
    assert_eq!(response.status(), StatusCode::OK);
    let admin: model::contract::TokenResponse =
        response.json().await.expect("joined administration token");
    assert_eq!(
        http.get(format!("{}/applications", intent.idp))
            .bearer_auth(&admin.access_token.0)
            .send()
            .await
            .expect("joined IdP administration")
            .status(),
        StatusCode::OK,
        "fresh and source-offline Join retain imported identity grants"
    );

    assert_eq!(
        devices.as_array().expect("Device list").len(),
        2,
        "only actual source and joining Management Devices"
    );
    let started = Instant::now();
    let response = http
        .get(format!("{}/databases", intent.storage))
        .bearer_auth(storage)
        .timeout(Duration::from_secs(15))
        .send()
        .await;
    eprintln!(
        "Independent Join Storage elapsed_ms={} status={:?}",
        started.elapsed().as_millis(),
        response.as_ref().map(|response| response.status())
    );
    let response = response.expect("normal joined Storage within end-to-end request budget");
    assert_eq!(response.status(), StatusCode::OK);
    let catalog: serde_json::Value = response
        .json()
        .await
        .expect("joined empty resource namespace");
    assert!(catalog.as_array().expect("catalog").is_empty());
    devices
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "parent isolates native roots and secure provider"]
async fn independent_join_child() {
    let root = PathBuf::from(std::env::var_os("OF_INDEPENDENT_JOIN_ROOT").expect("fixture roots"));
    crate::support::timing::capture_setup_timings();
    let reset_provider = crate::owner_composition_tests::reset::Provider::install();
    let addresses: Vec<SocketAddr> =
        serde_json::from_slice(&fs::read(root.join("addresses.json")).expect("addresses"))
            .expect("fixture metadata");
    let source = ExistingSource::provision(&root.join("source"), &addresses).await;
    let source_shutdown = CancellationToken::new();
    let source_servers = source.serve(&addresses[..3], &source_shutdown).await;
    let http = LocalTls::verification_client().expect("native verified HTTPS");
    let (management_token, storage_token) = source_tokens(&http, &source).await;
    assert_eq!(
        http.put(format!(
            "{}/independent/node-joins/acceptance",
            source.device
        ))
        .json(&serde_json::json!({"accepting":true}))
        .send()
        .await
        .expect("unauthorized acceptance")
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        http.put(format!(
            "{}/independent/node-joins/acceptance",
            source.device
        ))
        .bearer_auth(&management_token)
        .json(&serde_json::json!({"accepting":true}))
        .send()
        .await
        .expect("source ordinary User opens Join acceptance")
        .status(),
        StatusCode::NO_CONTENT
    );
    let target = root.join("target");
    let mut intent = IndependentCreateIntent {
        purpose: "join".into(),
        operation_nonce: "0123456789abcdef0123456789abcdef".into(),
        administrator: String::new(),
        node: "Independent joining Node".into(),
        idp: format!("https://{}/idp", addresses[3]),
        device: format!("https://{}/device", addresses[4]),
        storage: format!("https://{}/storage", addresses[5]),
        issuer: String::new(),
        peer: Some(format!(
            "{}#{}",
            source.device,
            source.identity.endpoint_id()
        )),
    };
    let shutdown = CancellationToken::new();
    let servers = start(&target, &addresses[3..], &shutdown).await;
    wait(&http, &intent, false).await;
    let idp_code = setup_code(&target, "idp");
    let mut unretired_idp = None;
    for owner in ["idp", "device", "storage"] {
        let base = intent.base(owner).expect("owner");
        let code = setup_code(&target, owner);
        assert_eq!(
            http.post(format!("{base}/independent/setup/join"))
                .json(&intent)
                .send()
                .await
                .expect("unadmitted Join")
                .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            http.post(format!("{base}/independent/setup/join"))
                .header("Authorization", format!("Setup {code}"))
                .json(&intent)
                .send()
                .await
                .expect("host-owned Join acceptance")
                .status(),
            StatusCode::ACCEPTED
        );
        if owner == "idp" {
            unretired_idp = Some(
                fs::read(target.join("idp.setup-host/setup-admission/state"))
                    .expect("bound Join admission snapshot"),
            );
        }
    }
    intent.issuer = source.issuer.clone();
    let first = pending_offer(&http, &source, &management_token).await;
    for (owner, absent) in [("idp", "credential"), ("device", "create")] {
        let record: serde_json::Value = serde_json::from_slice(
            &fs::read(target.join(format!("{owner}.setup-host/worker.json")))
                .expect("durable admitted Join record"),
        )
        .expect("worker JSON");
        assert_eq!(record["version"], 2);
        assert!(
            record.get(absent).is_none(),
            "Join must not retain fresh Create-only IDs"
        );
        assert!(record.get("role").is_none());
        assert!(record.get("permissions").is_none());
    }
    let first_id = first["offer_id"].as_str().expect("exact offer");
    assert_ne!(
        first["request"]["device_id"], first["request"]["member_id"],
        "Device and signer membership are distinct"
    );
    assert_eq!(
        http.post(format!(
            "{}/independent/node-joins/{first_id}/approve",
            source.device
        ))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("unauthorized source approval")
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        http.post(format!(
            "{}/independent/node-joins/{first_id}/reject",
            source.device
        ))
        .bearer_auth(&management_token)
        .send()
        .await
        .expect("source exact rejection")
        .status(),
        StatusCode::NO_CONTENT
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let status: serde_json::Value = http
            .get(format!("{}/independent/setup/status", intent.device))
            .send()
            .await
            .expect("rejection observation")
            .json()
            .await
            .expect("status");
        if status["state"] == "unfinished" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "bounded rejected Join"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for owner in ["idp", "device", "storage"] {
        let base = intent.base(owner).expect("owner");
        let code = setup_code(&target, owner);
        let recovered: serde_json::Value = http
            .post(format!("{base}/independent/setup/intent"))
            .header("Authorization", format!("Setup {code}"))
            .send()
            .await
            .expect("authorized exact Join recovery")
            .json()
            .await
            .expect("nonsecret Join intent");
        assert_eq!(
            recovered,
            serde_json::to_value(&intent).expect("exact admitted discovery")
        );
        assert_eq!(
            http.post(format!("{base}/independent/setup/cancel"))
                .header("Authorization", format!("Setup {code}"))
                .json(&intent)
                .send()
                .await
                .expect("explicit Join cancellation")
                .status(),
            if owner == "device" {
                StatusCode::CONFLICT
            } else {
                StatusCode::NO_CONTENT
            }
        );
    }
    shutdown.cancel();
    for server in servers {
        server
            .await
            .expect("join interrupted owner")
            .expect("coordinated interruption");
    }
    let shutdown = CancellationToken::new();
    let servers = start(&target, &addresses[3..], &shutdown).await;
    wait(&http, &intent, false).await;
    for owner in ["idp", "device", "storage"] {
        let base = intent.base(owner).expect("owner");
        let code = setup_code(&target, owner);
        let mut conflicting = intent.clone();
        conflicting.node = "Competing name".into();
        assert_eq!(
            http.post(format!("{base}/independent/setup/join"))
                .header("Authorization", format!("Setup {code}"))
                .json(&conflicting)
                .send()
                .await
                .expect("conflicting Join")
                .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            http.post(format!("{base}/independent/setup/join"))
                .header("Authorization", format!("Setup {code}"))
                .json(&intent)
                .send()
                .await
                .expect("exact retained Join retry")
                .status(),
            StatusCode::ACCEPTED
        );
    }
    let retry = pending_offer(&http, &source, &management_token).await;
    assert_eq!(
        retry["request"], first["request"],
        "rejection/restart retry preserves exact endpoint, signer and owner bases"
    );
    let id = retry["offer_id"].as_str().expect("new offer ID");
    assert_ne!(id, first_id);
    let response = http
        .post(format!(
            "{}/independent/node-joins/{id}/approve",
            source.device
        ))
        .bearer_auth(&management_token)
        .send()
        .await
        .expect("source User approves exact offer");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    wait(&http, &intent, true).await;
    let devices = use_joined(&http, &intent, &management_token, &storage_token).await;
    let public = wait_public(&http, &intent.idp).await;
    assert_eq!(public["facts"]["approval"]["request"], first["request"]);
    assert_eq!(public["operation"]["issuer"], source.issuer);
    assert_ne!(
        public["facts"]["endpoint"],
        source.identity.endpoint_id().to_string()
    );
    let target_login = public["facts"]["login_client"]
        .as_str()
        .expect("target public login");
    let password = http
        .post(format!("{}/oauth2/token", intent.idp))
        .form(&[
            ("grant_type", "password"),
            ("client_id", target_login),
            ("username", "source-operator"),
            ("password", "source-protected-password"),
            ("scope", "openid"),
            ("resource", "idp-management"),
        ])
        .send()
        .await
        .expect("no imported local password verifier");
    assert!(
        !password.status().is_success(),
        "Join never replays administrator credentials"
    );
    let secrets = forbidden_secrets(&target, &http, &intent).await;
    for owner in ["idp", "device", "storage"] {
        let base = intent.base(owner).expect("owner");
        let body = http
            .get(format!("{base}/independent/setup/status"))
            .send()
            .await
            .expect("public status")
            .text()
            .await
            .expect("public JSON");
        assert!(secrets.iter().all(|secret| !body.contains(secret)));
    }
    source_shutdown.cancel();
    for server in source_servers {
        server
            .await
            .expect("source listener")
            .expect("source HTTPS stopped");
    }
    source
        .management_runtime
        .shutdown()
        .await
        .expect("source pairing shutdown");
    source
        .storage_runtime
        .shutdown()
        .await
        .expect("source Storage shutdown");
    source.identity.endpoint().close().await;
    drop(source.idp_runtime);
    shutdown.cancel();
    for server in servers {
        server
            .await
            .expect("target stop")
            .expect("target coordinated shutdown");
    }
    fs::write(
        target.join("idp.setup-host/setup-admission/state"),
        unretired_idp.expect("unretired checkpoint"),
    )
    .expect("inject completed-before-retirement Join interruption");
    let shutdown = CancellationToken::new();
    let servers = start(&target, &addresses[3..], &shutdown).await;
    wait_public(&http, &intent.idp).await;
    let status: serde_json::Value = http
        .get(format!("{}/independent/setup/status", intent.idp))
        .send()
        .await
        .expect("unfinished offline Join")
        .json()
        .await
        .expect("status");
    assert_eq!(status["ready"], false);
    let recovered: serde_json::Value = http
        .post(format!("{}/independent/setup/intent", intent.idp))
        .header("Authorization", format!("Setup {idp_code}"))
        .send()
        .await
        .expect("offline exact finalization recovery")
        .json()
        .await
        .expect("nonsecret recovery");
    assert_eq!(
        recovered,
        serde_json::to_value(&intent).expect("exact source issuer and peer")
    );
    assert_eq!(
        http.post(format!("{}/independent/setup/join", intent.idp))
            .header("Authorization", format!("Setup {idp_code}"))
            .json(&intent)
            .send()
            .await
            .expect("source-offline exact Join finalization")
            .status(),
        StatusCode::ACCEPTED
    );
    wait(&http, &intent, true).await;
    assert_eq!(
        use_joined(&http, &intent, &management_token, &storage_token).await,
        devices,
        "source-offline reopen retains imported owner authority and exact endpoint"
    );
    shutdown.cancel();
    for server in servers {
        server
            .await
            .expect("reopen listener")
            .expect("source-offline target shutdown");
    }
    crate::owner_composition_tests::reset::qualify_completed(
        &target,
        &reset_provider,
        &intent.issuer,
    )
    .await;
    assert!(
        root.join("source/idp/idp.redb").exists(),
        "target reset preserves the source"
    );
}
async fn forbidden_secrets(
    root: &Path,
    http: &Client,
    intent: &IndependentCreateIntent,
) -> Vec<String> {
    let mut secrets = vec!["source-protected-password".into()];
    for owner in ["idp", "device", "storage"] {
        let public = wait_public(http, intent.base(owner).expect("owner")).await;
        let provider = idp_service::repo::RawKeyringRepo::new(format!(
            "independent-setup:{}",
            root.join(format!("{owner}.setup-host")).display()
        ))
        .expect("isolated provider");
        for client in public["clients"].as_array().expect("public registrations") {
            secrets.push(
                String::from_utf8(
                    provider
                        .load(
                            "",
                            "",
                            client["client_id"].as_str().expect("public Client ID"),
                        )
                        .expect("protected read")
                        .expect("stable consumer secret"),
                )
                .expect("secret text"),
            );
        }
        assert!(
            public["clients"]
                .as_array()
                .expect("registrations")
                .iter()
                .all(|registration| registration["client_secret"].is_null())
        );
    }
    for owner in ["idp", "device", "storage"] {
        let public = wait_public(http, intent.base(owner).expect("owner"))
            .await
            .to_string();
        let record = fs::read_to_string(root.join(format!("{owner}.setup-host/worker.json")))
            .expect("nonsecret durable owner evidence");
        assert!(
            secrets
                .iter()
                .all(|secret| !public.contains(secret) && !record.contains(secret)),
            "no password or service credential in browser-visible facts or durable worker records"
        );
    }
    secrets
}
