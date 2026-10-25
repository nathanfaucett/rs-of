use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use of_idp_client::{
    apis::{
        configuration::{ApiKey, Configuration},
        default_api as idp, independent_setup_api as independent, setup_api as setup,
    },
    models,
};
use of_storage_client::{apis::resources_api as storage, models::CreateResourceRequest};
use serde_json::json;

#[path = "authorization.rs"]
mod authorization;
#[path = "recovery.rs"]
mod recovery;

pub use authorization::denial_scenario;
use recovery::{
    JoinAdmission, assert_learned_grants, denied_refresh, pending_offer, prepare_learned_grants,
    refresh_grant, retry_rejected_join,
};

const PASSWORD: &str = "system-test-initial-password";
const NONCE: &str = "0123456789abcdef0123456789abcdef";

struct Process {
    child: Child,
    name: String,
    log: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(log) = self.log.take() {
            let _ = log.join();
        }
    }
}

struct Node {
    processes: Vec<Process>,
    commands: Vec<(String, Command)>,
    sql: Option<SocketAddr>,
    root: tempfile::TempDir,
    http: reqwest::Client,
    idp: String,
    management: String,
    device: String,
    storage: String,
    issuer: String,
    login: String,
}

impl Drop for Node {
    fn drop(&mut self) {
        while let Some(process) = self.processes.pop() {
            drop(process);
        }
        if std::thread::panicking() {
            self.root.disable_cleanup(true);
            eprintln!(
                "private system-test failure diagnostics retained at {}",
                self.root.path().display()
            );
        }
    }
}

fn address() -> SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .expect("reserve loopback port")
        .local_addr()
        .expect("loopback address")
}

fn binaries() -> PathBuf {
    std::env::current_exe()
        .expect("test executable")
        .parent()
        .expect("deps directory")
        .parent()
        .expect("target profile")
        .to_owned()
}

#[test]
fn root_is_private() {
    const CHILD: &str = "OF_SYSTEM_ROOT_PERMISSIONS_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let node = Node::new();
        let mode = fs::metadata(node.root.path())
            .expect("system-test root metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "system-test root must exclude group/other");
        return;
    }

    let status = Command::new("sh")
        .arg("-c")
        .arg("umask 000; exec \"$0\" --exact create::root_is_private --nocapture")
        .arg(std::env::current_exe().expect("test executable"))
        .env(CHILD, "1")
        .status()
        .expect("run root permission regression with permissive child umask");
    assert!(status.success(), "root permission regression child failed");
}

impl Node {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .expect("private system-test root");
        let tools = root.path().join("tools");
        fs::create_dir(&tools).expect("private test tools");
        for name in ["trust", "pkexec"] {
            let file = tools.join(name);
            fs::write(&file, "#!/bin/sh\nexit 0\n").expect("mock OS trust command");
            fs::set_permissions(file, fs::Permissions::from_mode(0o700)).expect("test tool mode");
        }
        fs::create_dir(root.path().join("empty-certs")).expect("isolated native roots");
        Self {
            processes: Vec::new(),
            commands: Vec::new(),
            sql: None,
            root,
            http: client(None),
            idp: String::new(),
            management: String::new(),
            device: String::new(),
            storage: String::new(),
            issuer: String::new(),
            login: String::new(),
        }
    }

    fn start(&mut self, name: &str, executable: &Path, args: &[String], env: &[(&str, String)]) {
        assert!(
            executable.is_file(),
            "build prerequisite missing: {}",
            executable.display()
        );
        let mut command = Command::new(executable);
        command
            .args(args)
            .current_dir(self.root.path())
            .env("OF_SYSTEM_SERVICE", name)
            .env(
                "OF_SYSTEM_PROVIDER",
                self.root.path().join(format!("{name}.provider")),
            )
            .env("RUST_MIN_STACK", "67108864")
            .env("RUST_LOG", "warn,idp_server::independent_create=debug,idp_server::independent_create::join=debug,device_server::independent_create=debug,device_server::independent_create::join=debug,storage_server::idp_client=debug")
            .env("SSL_CERT_FILE", self.root.path().join("roots.pem"))
            .env("SSL_CERT_DIR", self.root.path().join("empty-certs"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.root.path().join("tools").display(),
                    std::env::var("PATH").expect("PATH")
                ),
            )
            .env_remove("PORTLESS_URL")
            .env_remove("PORT")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        for (key, _) in std::env::vars_os() {
            if ["SERVER_", "LIDP_", "STORAGE_", "UNIFIED_"]
                .iter()
                .any(|prefix| key.to_string_lossy().starts_with(prefix))
            {
                command.env_remove(key);
            }
        }
        for (key, value) in env {
            command.env(key, value);
        }
        self.processes
            .push(Self::spawn(name, &mut command, self.root.path()));
        self.commands.push((name.into(), command));
    }

    fn spawn(name: &str, command: &mut Command, root: &Path) -> Process {
        let mut log =
            fs::File::create(root.join(format!("{name}.log"))).expect("private child log");
        let mut child = command.spawn().expect("start production service");
        let mut stderr = child.stderr.take().expect("piped service diagnostics");
        let reader = std::thread::spawn(move || {
            let mut remaining = 16 * 1024;
            let mut buffer = [0; 1024];
            while let Ok(count) = stderr.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                let kept = count.min(remaining);
                let _ = log.write_all(&buffer[..kept]);
                remaining -= kept;
            }
        });
        Process {
            child,
            name: name.into(),
            log: Some(reader),
        }
    }

    async fn restart(&mut self, independent_layout: bool) {
        while let Some(process) = self.processes.pop() {
            drop(process);
        }
        for i in 0..self.commands.len() {
            if self.commands[i].0 == "management" && self.sql.is_none() {
                let deadline = Instant::now() + Duration::from_secs(60);
                loop {
                    self.alive();
                    if independent::independent_setup_status(&self.config(&self.idp))
                        .await
                        .is_ok_and(|status| status.ready)
                    {
                        let public =
                            independent::independent_setup_owner_public(&self.config(&self.idp))
                                .await
                                .expect("reopened local IdP evidence");
                        let facts = public.facts.expect("reopened local IdP facts");
                        if tokio::net::UnixStream::connect(
                            facts["identity_socket"]
                                .as_str()
                                .expect("retained explicit owner socket"),
                        )
                        .await
                        .is_ok()
                        {
                            break;
                        }
                    }
                    assert!(
                        Instant::now() < deadline,
                        "retained IdP socket startup timeout"
                    );
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
            let (name, command) = &mut self.commands[i];
            self.processes
                .push(Self::spawn(name, command, self.root.path()));
            if name == "identity-database" {
                let deadline = Instant::now() + Duration::from_secs(30);
                while tokio::net::TcpStream::connect(self.sql.expect("SQL address"))
                    .await
                    .is_err()
                {
                    assert!(
                        Instant::now() < deadline,
                        "retained SQL-owner startup timeout"
                    );
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            self.alive();
            let ready = if independent_layout {
                let mut ready = true;
                for base in [&self.idp, &self.device, &self.storage] {
                    ready &= independent::independent_setup_status(&self.config(base))
                        .await
                        .is_ok_and(|s| s.ready);
                }
                ready
            } else {
                setup::setup_status(&self.config(&self.idp))
                    .await
                    .is_ok_and(|s| s.ready)
            };
            if ready {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "retained restart readiness timeout"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    fn alive(&mut self) {
        for process in &mut self.processes {
            assert!(
                process.child.try_wait().expect("child status").is_none(),
                "{} exited; private diagnostics: {}",
                process.name,
                self.root.path().display()
            );
        }
    }

    fn config(&self, base: &str) -> Configuration {
        Configuration {
            client: self.http.clone(),
            base_path: base.into(),
            ..Configuration::default()
        }
    }

    fn write_config(&self, owner: &str, value: serde_json::Value) -> String {
        let file = self.root.path().join(format!("{owner}.yaml"));
        fs::write(
            &file,
            serde_json::to_vec(&value).expect("configuration JSON is valid YAML"),
        )
        .expect("write configuration");
        file.to_string_lossy().into_owned()
    }

    async fn start_unified(&mut self) -> Configuration {
        let addr = address();
        let base = format!("http://{addr}");
        self.idp = format!("{base}/idp");
        self.management = format!("{base}/management");
        self.device = format!("{base}/device");
        self.storage = format!("{base}/storage");
        let domain = self.root.path().join("unified");
        self.start(
            "unified",
            &binaries().join("examples/system-test-service"),
            &[
                "--data-dir".into(),
                domain.to_string_lossy().into_owned(),
                "serve".into(),
                "--port".into(),
                addr.port().to_string(),
            ],
            &[],
        );
        let deadline = Instant::now() + Duration::from_secs(30);
        let admission = domain.join("setup-admission/state");
        loop {
            self.alive();
            if admission.exists() && setup::setup_status(&self.config(&self.idp)).await.is_ok() {
                break;
            }
            assert!(Instant::now() < deadline, "Unified setup startup timeout");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let mut config = self.config(&self.idp);
        config.api_key = Some(capability(&admission));
        assert!(
            !setup::setup_status(&config)
                .await
                .expect("fresh Unified status")
                .ready
        );
        config
    }

    async fn unified(&mut self) {
        let config = self.start_unified().await;
        let request = models::CreateRequest::new(
            "operator".into(),
            "System Node".into(),
            NONCE.into(),
            PASSWORD.into(),
        );
        setup::setup_create(
            &config,
            setup::SetupCreateParams {
                create_request: request.clone(),
            },
        )
        .await
        .unwrap_or_else(|_| panic!("Unified Create admission failed"));
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            self.alive();
            let observed = setup::setup_status(&config).await;
            if observed.as_ref().is_ok_and(|s| s.ready) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Unified readiness timeout: {observed:?}"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let result = setup::setup_create(
            &config,
            setup::SetupCreateParams {
                create_request: request,
            },
        )
        .await
        .unwrap_or_else(|_| panic!("completed exact Create verification failed"));
        assert!(result.ready);
        self.login = result
            .login_client
            .flatten()
            .expect("public operator login Client")
            .client_id;
        self.issuer = idp::openid_configuration(&self.config(&self.idp))
            .await
            .expect("public Installation discovery")
            .issuer;
    }

    async fn start_independent(&mut self, local: bool, source_roots: &[u8]) -> Vec<ApiKey> {
        let sql = address();
        self.sql = (!local).then_some(sql);
        let addrs = [address(), address(), address()];
        self.idp = format!("https://{}", addrs[0]);
        self.device = format!("https://{}/device", addrs[1]);
        self.storage = format!("https://{}/storage", addrs[2]);
        self.issuer = self.idp.clone();
        let mut roots = source_roots.to_vec();
        let mut capabilities = Vec::new();
        for ((owner, prefix), addr) in [("idp", ""), ("device", "/device"), ("storage", "/storage")]
            .into_iter()
            .zip(addrs)
        {
            let host = api::IndependentSetupHost::prepare(
                owner,
                &self.root.path().join(owner),
                addr,
                "https://localhost:5173",
                prefix,
                false,
            )
            .expect("prepare protected host")
            .expect("fresh owner");
            roots.extend(fs::read(host.tls().ca_path()).expect("test CA"));
            capabilities.push(capability(&host.admission_path()));
        }
        fs::write(self.root.path().join("roots.pem"), &roots).expect("isolated native CA bundle");
        self.http = client(Some(&roots));
        if !local {
            self.start(
                "identity-database",
                &binaries().join("identity-database"),
                &[
                    "--database".into(),
                    self.root
                        .path()
                        .join("identity.redb")
                        .to_string_lossy()
                        .into_owned(),
                    "--listen".into(),
                    sql.to_string(),
                ],
                &[],
            );
            let deadline = Instant::now() + Duration::from_secs(30);
            while tokio::net::TcpStream::connect(sql).await.is_err() {
                self.alive();
                assert!(Instant::now() < deadline, "SQL-owner startup timeout");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        for (i, (owner, prefix, base)) in [
            ("idp", "", self.idp.clone()),
            ("device", "/device", self.device.clone()),
            ("storage", "/storage", self.storage.clone()),
        ]
        .into_iter()
        .enumerate()
        {
            let mut value = json!({"data_dir":self.root.path().join(owner), "log_level":"warn", "server":{"host":"127.0.0.1", "port":addrs[i].port(), "prefix":prefix, "setup_ui_origin":"https://localhost:5173"}, "api_public_uri":base});
            if owner != "storage" && !local {
                value["identity_database"] =
                    json!({"endpoint":format!("http://{sql}"), "deadline_ms":20000});
            }
            if owner == "idp" {
                value["ui_public_uri"] = json!("https://localhost:5173");
                value["password"] = json!({"memory_mib":1, "iterations":1});
            }
            let file = self.write_config(owner, value);
            self.start(
                owner,
                &binaries().join("examples/system-test-service"),
                &["--config".into(), file, "serve".into()],
                &[],
            );
        }
        capabilities
    }

    async fn independent(&mut self) {
        self.independent_create(false).await;
    }

    async fn independent_create(&mut self, local: bool) {
        let capabilities = self.start_independent(local, &[]).await;
        let request = models::IndependentCreateRequest::new(
            "operator".into(),
            self.device.clone(),
            self.idp.clone(),
            self.issuer.clone(),
            "System Node".into(),
            NONCE.into(),
            "create".into(),
            self.storage.clone(),
        );
        for (i, base) in [self.idp.clone(), self.device.clone(), self.storage.clone()]
            .into_iter()
            .enumerate()
        {
            let mut config = self.config(&base);
            config.api_key = Some(capabilities[i].clone());
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                self.alive();
                if let Ok(status) = independent::independent_setup_status(&config).await {
                    assert!(!status.ready);
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "independent owner startup timeout"
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let mut owner_request = request.clone();
            if i == 0 {
                owner_request.password = Some(Some(PASSWORD.into()));
            }
            independent::independent_setup_create(
                &config,
                independent::IndependentSetupCreateParams {
                    independent_create_request: owner_request,
                },
            )
            .await
            .unwrap_or_else(|_| panic!("independent owner {i} rejected Create"));
        }
        let mut evidence = Vec::new();
        for (owner, base) in [
            ("idp", self.idp.clone()),
            ("device", self.device.clone()),
            ("storage", self.storage.clone()),
        ] {
            let config = self.config(&base);
            let deadline = Instant::now() + Duration::from_secs(180);
            loop {
                self.alive();
                let observed = independent::independent_setup_status(&config).await;
                if let Ok(status) = &observed
                    && status.ready
                {
                    assert_eq!(status.owners, [owner]);
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "{owner} readiness timeout: {observed:?}"
                );
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            let public = independent::independent_setup_owner_public(&config)
                .await
                .expect("public correlated owner evidence");
            assert_eq!(public.owner, owner);
            assert_eq!(public.operation.operation_nonce, NONCE);
            assert_eq!(public.operation.issuer, self.issuer);
            assert_eq!(public.operation.idp, self.idp);
            assert_eq!(public.operation.device, self.device);
            assert_eq!(public.operation.storage, self.storage);
            assert_eq!(public.operation.administrator, "operator");
            assert_eq!(public.operation.node, "System Node");
            assert_eq!(public.operation.purpose, "create");
            assert!(public.operation.peer.as_ref().is_none_or(Option::is_none));
            evidence.push(public.facts.expect("current public owner facts"));
        }
        assert_eq!(evidence[0]["endpoint"], evidence[1]["endpoint"]);
        assert_eq!(evidence[1]["endpoint"], evidence[2]["endpoint"]);
        assert!(
            evidence[0]["endpoint"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        );
        self.login = evidence[0]["login_client"]
            .as_str()
            .expect("published operator Client")
            .into();
        self.start_management(None).await;
    }

    async fn start_management(&mut self, authority: Option<&str>) {
        let bearer = if let Some(authority) = authority {
            authority.to_owned()
        } else {
            let login = self.login().await;
            self.exchange(&login, &self.idp, "").await
        };
        let mut config = self.config(&self.idp);
        config.bearer_access_token = Some(bearer);
        let application = models::ApplicationRegistration {
            uri: format!("{}/system-test-policy", self.idp),
            name: Some(Some("System-test policy relationship".into())),
            ..Default::default()
        };
        idp::create_application(
            &config,
            idp::CreateApplicationParams {
                application_registration: application.clone(),
            },
        )
        .await
        .unwrap_or_else(|_| panic!("ordinary relationship Application registration failed"));
        let registration = models::ClientRegistration {
            application: Box::new(application),
            client_name: "system-test Management relationship".into(),
            profile: models::ClientProfile::WebApplication,
            client_type: Some(models::ClientType::Confidential),
            allowed_grant_types: Some(vec![models::GrantType::ClientCredentials]),
            allowed_scopes: Some(vec!["idp.token.validate".into(), "idp.user.lookup".into()]),
            allowed_audiences: Some(vec![self.idp.clone(), "idp-management".into()]),
            token_endpoint_auth_method: models::TokenEndpointAuthMethod::ClientSecretPost,
            ..Default::default()
        };
        let registration = idp::register(
            &config,
            idp::RegisterParams {
                client_registration: registration,
            },
        )
        .await
        .unwrap_or_else(|_| panic!("ordinary Management service-client registration failed"));
        let addr = address();
        self.management = format!("http://{addr}/policy");
        let mut value = json!({"data_dir":self.root.path().join("management"), "server":{"host":"127.0.0.1", "port":addr.port(), "prefix":"/policy"}, "api_public_uri":self.management, "idp_api_base":self.idp, "storage_api_base":self.storage, "expected_issuer":self.issuer});
        if let Some(sql) = self.sql {
            value["identity_database"] =
                json!({"endpoint":format!("http://{sql}"), "deadline_ms":20000});
        } else {
            let public = independent::independent_setup_owner_public(&self.config(&self.idp))
                .await
                .expect("local IdP public connection evidence");
            assert_eq!(public.owner, "idp");
            assert_eq!(public.operation.issuer, self.issuer);
            assert_eq!(public.operation.idp, self.idp);
            let facts = public.facts.expect("local IdP facts");
            value["identity_socket"] = json!(
                facts["identity_socket"]
                    .as_str()
                    .expect("explicit local-owner socket")
            );
            value["identity_socket_deadline_ms"] = json!(20000);
        }
        let file = self.write_config("management", value);
        self.start(
            "management",
            &binaries().join("examples/system-test-service"),
            &["--config".into(), file, "serve".into()],
            &[
                (
                    "SERVER_IDP_OAUTH_CLIENT_ID",
                    registration
                        .client_id
                        .flatten()
                        .expect("registered Client ID"),
                ),
                (
                    "SERVER_IDP_OAUTH_CLIENT_SECRET",
                    registration
                        .client_secret
                        .flatten()
                        .expect("registered Client secret"),
                ),
                ("SERVER_IDP_SERVICE_AUDIENCE", self.idp.clone()),
            ],
        );
        self.management_ready().await;
    }

    async fn policy_permission(&self, authority: &str, subject: &str) {
        let config = of_management_client::apis::configuration::Configuration {
            client: self.http.clone(),
            base_path: self.management.clone(),
            bearer_access_token: Some(authority.into()),
            ..Default::default()
        };
        let request: of_management_client::models::PermissionEvaluationRequest = serde_json::from_value(json!({
            "requestId":"01234567-89ab-4cde-8fab-0123456789ab", "action":"devices_read", "subject":{"type":"user","id":subject}, "target":{"scope":"installation","resource":{"type":"device","id":null}}
        })).expect("generated permission request model");
        let result = of_management_client::apis::default_api::evaluate_permission(
            &config,
            of_management_client::apis::default_api::EvaluatePermissionParams {
                permission_evaluation_request: request.clone(),
            },
        )
        .await
        .unwrap_or_else(|error| {
            panic!(
                "authorized Management permission evaluation failed: {}",
                match error {
                    of_management_client::apis::Error::ResponseError(response) =>
                        format!("HTTP {}", response.status),
                    _ => "transport or decoding".into(),
                }
            )
        });
        assert!(result.allowed);
        assert_eq!(*result.request, request);
    }

    async fn management_ready(&mut self) {
        let config = of_management_client::apis::configuration::Configuration {
            client: self.http.clone(),
            base_path: self.management.clone(),
            ..Default::default()
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            self.alive();
            if of_management_client::apis::default_api::health(&config)
                .await
                .is_ok()
            {
                break;
            }
            assert!(Instant::now() < deadline, "Management startup timeout");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn login(&self) -> String {
        self.login_response().await.access_token
    }

    async fn login_response(&self) -> models::TokenResponse {
        idp::token(
            &self.config(&self.idp),
            idp::TokenParams {
                grant_type: Some("password".into()),
                client_id: Some(self.login.clone()),
                username: Some("operator".into()),
                password: Some(PASSWORD.into()),
                scope: Some("openid".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap_or_else(|error| {
            panic!(
                "ordinary Initial Administrator login failed: {}",
                match error {
                    of_idp_client::apis::Error::ResponseError(response) =>
                        format!("HTTP {}", response.status),
                    _ => "transport or decoding".into(),
                }
            )
        })
    }

    async fn exchange(&self, token: &str, audience: &str, scope: &str) -> String {
        idp::token(&self.config(&self.idp), idp::TokenParams { grant_type:Some("urn:ietf:params:oauth:grant-type:token-exchange".into()), client_id:Some(self.login.clone()), subject_token:Some(token.into()), subject_token_type:Some(models::SubjectTokenType::UrnColonIetfColonParamsColonOauthColonTokenTypeColonAccessToken), resource:Some(audience.into()), scope:(!scope.is_empty()).then(|| scope.into()), authorization_details:(audience == "storage:filesystems").then(|| r#"[{"type":"storage","actions":["read","write"]}]"#.into()), ..Default::default() }).await.unwrap_or_else(|_| panic!("ordinary User token exchange failed")).access_token
    }
}

fn capability(path: &Path) -> ApiKey {
    let state = fs::read_to_string(path).expect("protected host-management retrieval");
    ApiKey {
        prefix: Some("Setup".into()),
        key: state.lines().nth(1).expect("host capability").into(),
    }
}

fn client(roots: Option<&[u8]>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(30));
    if let Some(roots) = roots {
        builder = builder.tls_built_in_root_certs(false);
        for cert in reqwest::Certificate::from_pem_bundle(roots).expect("test CA bundle") {
            builder = builder.add_root_certificate(cert);
        }
    }
    builder.build().expect("verified bounded client")
}

pub async fn scenario(independent_layout: bool) {
    create_node(independent_layout, false, false).await;
}

pub async fn restart_scenario(independent_layout: bool) {
    create_node(independent_layout, true, false).await;
}

pub async fn local_restart_scenario() {
    create_node(true, true, true).await;
}

async fn create_node(independent_layout: bool, restart: bool, local: bool) {
    let mut node = Node::new();
    if independent_layout {
        if local {
            node.independent_create(true).await;
        } else {
            node.independent().await;
        }
    } else {
        node.unified().await;
    }
    let resource = use_node(&mut node, None, None).await;
    if restart {
        let endpoint = of_device_client::apis::default_api::device(&device_config(&node))
            .await
            .expect("created endpoint");
        node.restart(independent_layout).await;
        assert_eq!(
            of_device_client::apis::default_api::device(&device_config(&node))
                .await
                .expect("reopened endpoint")
                .public_key,
            endpoint.public_key
        );
        use_node(&mut node, None, Some(&resource)).await;
    }
}

async fn use_node(node: &mut Node, resource_token: Option<&str>, retained: Option<&str>) -> String {
    let metadata = idp::openid_configuration(&node.config(&node.idp))
        .await
        .expect("Installation discovery");
    assert_eq!(metadata.issuer, node.issuer);
    let device = of_device_client::apis::configuration::Configuration {
        client: node.http.clone(),
        base_path: node.device.clone(),
        ..Default::default()
    };
    of_device_client::apis::default_api::health(&device)
        .await
        .expect("Device database health");
    node.management_ready().await;
    let bearer = if let Some(token) = resource_token {
        token.to_owned()
    } else {
        let token = node.login().await;
        assert!(!token.is_empty());
        let mut user_config = node.config(&node.idp);
        user_config.bearer_access_token = Some(token.clone());
        let user = idp::userinfo(&user_config)
            .await
            .unwrap_or_else(|_| panic!("ordinary User info failed"));
        assert!(!user.sub.is_empty());
        node.policy_permission(
            &node.exchange(&token, "idp-management", "").await,
            &user.sub,
        )
        .await;
        node.exchange(&token, "storage:filesystems", "").await
    };
    let config = of_storage_client::apis::configuration::Configuration {
        client: node.http.clone(),
        base_path: node.storage.clone(),
        bearer_access_token: Some(bearer),
        ..Default::default()
    };
    if let Some(id) = retained {
        let read = storage::get_file_system(
            &config,
            storage::GetFileSystemParams {
                filesystem_id: id.into(),
            },
        )
        .await
        .unwrap_or_else(|_| panic!("retained authorized resource read failed"));
        assert_eq!(read.id, id);
        assert_eq!(
            read.name.as_ref().and_then(Option::as_deref),
            Some("Shared Create resource")
        );
    }
    let created = storage::create_file_system(
        &config,
        storage::CreateFileSystemParams {
            create_resource_request: CreateResourceRequest {
                name: Some(Some("Shared Create resource".into())),
            },
        },
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "authorized resource creation failed: {}",
            match error {
                of_storage_client::apis::Error::ResponseError(response) =>
                    format!("HTTP {}", response.status),
                _ => "transport or typed response decoding".into(),
            }
        )
    });
    assert_eq!(
        created.name.as_ref().and_then(Option::as_deref),
        Some("Shared Create resource")
    );
    let read = storage::get_file_system(
        &config,
        storage::GetFileSystemParams {
            filesystem_id: created.id.clone(),
        },
    )
    .await
    .unwrap_or_else(|_| panic!("authorized resource read failed"));
    assert_eq!(read.id, created.id);
    assert_eq!(read.name, created.name);
    let denied = of_storage_client::apis::configuration::Configuration {
        bearer_access_token: None,
        ..config
    };
    let error = storage::get_file_system(
        &denied,
        storage::GetFileSystemParams {
            filesystem_id: created.id.clone(),
        },
    )
    .await
    .expect_err("anonymous resource access must fail");
    assert!(
        matches!(error, of_storage_client::apis::Error::ResponseError(response) if matches!(response.status.as_u16(), 401 | 403))
    );
    node.alive();
    created.id
}

fn device_config(node: &Node) -> of_device_client::apis::configuration::Configuration {
    of_device_client::apis::configuration::Configuration {
        client: node.http.clone(),
        base_path: node.device.clone(),
        ..Default::default()
    }
}

pub async fn join_scenario(independent_layout: bool) {
    use of_device_client::apis::{
        independent_join_api as independent_joins, node_join_api as joins,
    };
    let mut source = Node::new();
    if independent_layout {
        source.independent_create(true).await;
    } else {
        source.unified().await;
    }
    let source_device = of_device_client::apis::default_api::device(&device_config(&source))
        .await
        .expect("public source endpoint");
    let grants = prepare_learned_grants(&source).await;
    let session = source.login().await;
    let authority = source.exchange(&session, "idp-management", "").await;
    let resource_token = source.exchange(&session, "storage:filesystems", "").await;
    let mut approval = device_config(&source);
    approval.bearer_access_token = Some(authority.clone());
    let anonymous = device_config(&source);
    if independent_layout {
        authorization_denied(
            independent_joins::independent_join_set_accepting(
                &anonymous,
                independent_joins::IndependentJoinSetAcceptingParams {
                    acceptance_request: of_device_client::models::AcceptanceRequest::new(true),
                },
            )
            .await,
        );
    } else {
        authorization_denied(
            joins::node_join_set_acceptance(
                &anonymous,
                joins::NodeJoinSetAcceptanceParams {
                    acceptance: of_device_client::models::Acceptance::new(true),
                },
            )
            .await,
        );
    }
    if independent_layout {
        independent_joins::independent_join_set_accepting(
            &approval,
            independent_joins::IndependentJoinSetAcceptingParams {
                acceptance_request: of_device_client::models::AcceptanceRequest::new(true),
            },
        )
        .await
        .unwrap_or_else(|_| panic!("authorized independent source acceptance"));
    } else {
        joins::node_join_set_acceptance(
            &approval,
            joins::NodeJoinSetAcceptanceParams {
                acceptance: of_device_client::models::Acceptance::new(true),
            },
        )
        .await
        .unwrap_or_else(|_| panic!("authorized source acceptance"));
    }
    if !independent_layout {
        tokio::time::timeout(Duration::from_secs(90), async {
            loop {
                source.alive();
                if let Ok(issuer) = idp_server::discover_installation_issuer(&source_device.public_key).await {
                    assert_eq!(issuer, source.issuer);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }).await.expect("native Trusted Peer discovery readiness timeout (production N0 discovery requires network access)");
    }
    let mut target = Node::new();
    let mut target_authority = None;
    let admission = if independent_layout {
        let roots =
            fs::read(source.root.path().join("roots.pem")).expect("source isolated CA trust");
        let capabilities = target.start_independent(true, &roots).await;
        let roots =
            fs::read(target.root.path().join("roots.pem")).expect("mutual isolated HTTPS trust");
        fs::write(source.root.path().join("roots.pem"), &roots)
            .expect("source verifies target history callbacks");
        source.http = client(Some(&roots));
        target_authority = Some(source.exchange(&session, &target.idp, "").await);
        let mut request = models::IndependentCreateRequest::new(
            String::new(),
            target.device.clone(),
            target.idp.clone(),
            String::new(),
            "Joining System Node".into(),
            NONCE.into(),
            "join".into(),
            target.storage.clone(),
        );
        request.peer = Some(format!("{}#{}", source.device, source_device.public_key));
        let mut configs = Vec::new();
        for (i, base) in [
            target.idp.clone(),
            target.device.clone(),
            target.storage.clone(),
        ]
        .iter()
        .enumerate()
        {
            let mut config = target.config(base);
            config.api_key = Some(capabilities[i].clone());
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                target.alive();
                if independent::independent_setup_status(&config).await.is_ok() {
                    break;
                }
                assert!(Instant::now() < deadline, "target worker startup timeout");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            configs.push(config);
        }
        JoinAdmission::Independent(configs, request)
    } else {
        let config = target.start_unified().await;
        JoinAdmission::Unified(
            config,
            models::JoinRequest::new(
                "Joining System Node".into(),
                NONCE.into(),
                source_device.public_key.clone(),
            ),
        )
    };
    admission.submit().await;
    let offer = pending_offer(&mut source, &mut target, &approval, independent_layout).await;
    assert_eq!(
        offer["authenticated_endpoint"],
        offer["request"]["endpoint_id"]
    );
    if independent_layout {
        assert_eq!(offer["request"]["operation"]["issuer"], source.issuer);
    } else {
        assert_eq!(offer["request"]["installation"], source.issuer);
    }
    let pending_ready = if independent_layout {
        independent::independent_setup_status(&target.config(&target.idp))
            .await
            .expect("pending independent target status")
            .ready
    } else {
        setup::setup_status(&target.config(&target.idp))
            .await
            .expect("pending target status")
            .ready
    };
    assert!(!pending_ready);
    let offer = retry_rejected_join(
        &mut source,
        &mut target,
        &approval,
        independent_layout,
        &admission,
        offer,
    )
    .await;
    let id = offer["offer_id"]
        .as_str()
        .expect("exact pending offer ID")
        .to_owned();
    if independent_layout {
        authorization_denied(
            independent_joins::independent_join_approve(
                &anonymous,
                independent_joins::IndependentJoinApproveParams { id: id.clone() },
            )
            .await,
        );
    } else {
        authorization_denied(
            joins::node_join_approve(&anonymous, joins::NodeJoinApproveParams { id: id.clone() })
                .await,
        );
    }
    if independent_layout {
        independent_joins::independent_join_approve(
            &approval,
            independent_joins::IndependentJoinApproveParams { id },
        )
        .await
        .unwrap_or_else(|_| panic!("authorized exact independent Device approval"));
    } else {
        let approved = joins::node_join_approve(&approval, joins::NodeJoinApproveParams { id })
            .await
            .unwrap_or_else(|_| panic!("authorized exact Device approval"));
        assert_eq!(
            serde_json::to_value(approved.request).expect("public approved request"),
            offer["request"]
        );
    }
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        source.alive();
        target.alive();
        if independent_layout {
            let mut ready = true;
            for base in [&target.idp, &target.device, &target.storage] {
                ready &= independent::independent_setup_status(&target.config(base))
                    .await
                    .is_ok_and(|s| s.ready);
            }
            if ready {
                break;
            }
        } else {
            let status = setup::setup_status(&target.config(&target.idp))
                .await
                .expect("target Join progress");
            if status.ready {
                target.login = status
                    .login_client
                    .flatten()
                    .expect("joined public operator login")
                    .client_id;
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "completed Join readiness timeout"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    target.issuer = source.issuer.clone();
    if independent_layout {
        let mut operations = Vec::new();
        let mut endpoints = Vec::new();
        for (owner, base) in [
            ("idp", &target.idp),
            ("device", &target.device),
            ("storage", &target.storage),
        ] {
            let public = independent::independent_setup_owner_public(&target.config(base))
                .await
                .expect("completed Join owner evidence");
            assert_eq!(public.owner, owner);
            assert_eq!(public.operation.operation_nonce, NONCE);
            assert_eq!(public.operation.issuer, source.issuer);
            assert_eq!(public.operation.node, "Joining System Node");
            assert_eq!(public.operation.purpose, "join");
            assert_eq!(public.operation.idp, target.idp);
            assert_eq!(public.operation.device, target.device);
            assert_eq!(public.operation.storage, target.storage);
            endpoints.push(public.facts.expect("completed owner facts")["endpoint"].clone());
            operations.push(public.operation);
        }
        assert_eq!(operations[0], operations[1]);
        assert_eq!(operations[1], operations[2]);
        assert_eq!(endpoints[0], endpoints[1]);
        assert_eq!(endpoints[1], endpoints[2]);
        assert!(endpoints[0].as_str().is_some_and(|value| !value.is_empty()));
        target.start_management(target_authority.as_deref()).await;
    }
    let mut user_config = source.config(&source.idp);
    user_config.bearer_access_token = Some(session);
    let subject = idp::userinfo(&user_config)
        .await
        .unwrap_or_else(|_| panic!("source public User info"))
        .sub;
    assert_learned_grants(&target, &source.login, &grants).await;
    let replacement = refresh_grant(&target, &source.login, &grants[2])
        .await
        .unwrap_or_else(|_| panic!("unconsumed learned grant remains usable"));
    assert!(
        replacement
            .refresh_token
            .flatten()
            .expect("rotated learned grant")
            != grants[2],
        "refresh rotation must replace the consumed learned grant"
    );
    denied_refresh(refresh_grant(&target, &source.login, &grants[2]).await);
    let resource = use_node(&mut target, Some(&resource_token), None).await;
    target.policy_permission(&authority, &subject).await;
    let endpoint = of_device_client::apis::default_api::device(&device_config(&target))
        .await
        .expect("joined endpoint");
    assert_ne!(endpoint.public_key, source_device.public_key);
    while let Some(process) = source.processes.pop() {
        drop(process);
    }
    target.restart(independent_layout).await;
    assert_eq!(
        of_device_client::apis::default_api::device(&device_config(&target))
            .await
            .expect("offline reopened endpoint")
            .public_key,
        endpoint.public_key
    );
    assert_learned_grants(&target, &source.login, &grants).await;
    denied_refresh(refresh_grant(&target, &source.login, &grants[2]).await);
    use_node(&mut target, Some(&resource_token), Some(&resource)).await;
    target.policy_permission(&authority, &subject).await;
}

fn authorization_denied<T, E>(result: Result<T, of_device_client::apis::Error<E>>) {
    assert!(
        matches!(result, Err(of_device_client::apis::Error::ResponseError(response)) if matches!(response.status.as_u16(), 401 | 403)),
        "source administration requires ordinary User authority"
    );
}
