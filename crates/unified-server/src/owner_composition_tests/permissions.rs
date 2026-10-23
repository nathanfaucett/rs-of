use std::{
    fs,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

use db::Database;
use device_service::HostedControlPlane;
use idp_model::{
    contract::{
        IdentityAction, IdentityResource, MANAGEMENT_PERMISSION_EVALUATE_SCOPE,
        PermissionEvaluationRequest, PermissionEvaluationResponse, PermissionSubject,
        PermissionTarget, TokenEndpointAuthMethod,
    },
    model::Id,
};
use idp_service::{
    replica::{DbApplicationRepo, DbClientRepo, DbKeyRepo},
    repo::{ApplicationRepo, ClientRepo, KeyService, PrivateKeyKeyringRepo},
};
use management_service::ManagementService;
use management_service::replica::DbPermissionRepo;
use management_service::replica::DbRoleRepo;
use reqwest::StatusCode;

use crate::permission_http_tests::{seed_idp, user_token};

struct TlsConfig {
    root: PathBuf,
    address: SocketAddr,
}

impl api::LocalTlsConfig for TlsConfig {
    fn tls_directory(&self) -> PathBuf {
        self.root.clone()
    }

    fn tls_address(&self) -> SocketAddr {
        self.address
    }
}

#[test]
fn independent_permission_production_management_https() {
    let root = std::env::temp_dir().join(format!("independent-permission-https-{}", Id::now_v7()));
    fs::create_dir(&root).expect("isolated native trust fixture");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private fixture");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve TLS address");
    let address = listener.local_addr().expect("TLS address");
    let mut bundle = Vec::new();
    for owner in ["idp", "device"] {
        let tls = api::LocalTls::create(&TlsConfig {
            root: root.join(owner),
            address,
        })
        .expect("independent owner TLS material");
        bundle.extend(fs::read(tls.ca_path()).expect("isolated owner CA"));
    }
    drop(listener);
    let ca_bundle = root.join("native-roots.pem");
    fs::write(&ca_bundle, bundle).expect("isolated native CA bundle");
    let empty_roots = root.join("empty-roots");
    fs::create_dir(&empty_roots).expect("isolated native CA directory");
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "owner_composition_tests::permissions::independent_permission_management_https_child",
            "--ignored",
            "--nocapture",
        ])
        .env("INDEPENDENT_PERMISSION_HTTPS_ROOT", &root)
        .env("SSL_CERT_FILE", &ca_bundle)
        .env("SSL_CERT_DIR", &empty_roots)
        .env("NO_PROXY", "127.0.0.1,::1,localhost")
        .stdin(Stdio::null())
        .spawn()
        .expect("isolated production HTTPS child");
    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
        if let Some(status) = child.try_wait().expect("HTTPS child status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("bound HTTPS child");
            child.wait().expect("reap HTTPS child");
            panic!("production Management permission child exceeded 120 seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        status.success(),
        "production Management permission HTTPS child"
    );
    fs::remove_dir_all(root).expect("isolated native trust cleanup");
}

async fn evaluator_token(
    client: &reqwest::Client,
    base: &str,
    client_id: &str,
    secret: &str,
) -> String {
    let response = client
        .post(format!("{base}/idp/oauth2/token"))
        .basic_auth(client_id, Some(secret))
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", secret),
            ("audience", management_service::MANAGEMENT_APPLICATION_URI),
            ("scope", MANAGEMENT_PERMISSION_EVALUATE_SCOPE),
        ])
        .send()
        .await
        .expect("production evaluator token endpoint");
    assert_eq!(response.status(), StatusCode::OK);
    response
        .json::<model::contract::TokenResponse>()
        .await
        .expect("typed evaluator token")
        .access_token
        .0
}

async fn decision(
    client: &reqwest::Client,
    endpoint: &str,
    bearer: &str,
    subject: Id,
    request: &PermissionEvaluationRequest,
) -> bool {
    let response = client
        .post(endpoint)
        .bearer_auth(bearer)
        .json(request)
        .send()
        .await
        .expect("production Management evaluation");
    assert_eq!(response.status(), StatusCode::OK);
    let response: PermissionEvaluationResponse =
        response.json().await.expect("typed exact decision");
    assert_eq!(response.request, *request);
    assert_eq!(response.audit.actor, request.subject);
    assert_eq!(response.audit.service_client_id, "idp-evaluator");
    assert_eq!(response.audit.service_subject, subject);
    response.allowed
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "parent supplies isolated native roots; never installs OS trust"]
async fn independent_permission_management_https_child() {
    tokio::time::timeout(Duration::from_secs(120), async {
        let root = PathBuf::from(
            std::env::var_os("INDEPENDENT_PERMISSION_HTTPS_ROOT").expect("parent HTTPS root"),
        );
        let owners = root.join("owners");
        let issuer = format!("https://permission-{}.test", Id::now_v7());
        let secret = iroh::SecretKey::generate();
        let server = iroh_chain::Server::bind_with_secret_key(
            iroh::endpoint::presets::Minimal,
            secret.clone(),
            iroh_chain::EndpointIdStore::default(),
        )
        .await
        .expect("isolated IdP endpoint");
        let fixture = seed_idp(
            &owners,
            &issuer,
            &server.endpoint().id().to_string(),
            &iroh::SecretKey::generate().public().to_string(),
        )
        .await;
        let idp_engine =
            Arc::new(Database::open(owners.join("idp/idp.redb")).expect("IdP owner store"));
        let keys = Arc::new(KeyService::new(
            DbKeyRepo::new(idp_engine.client()),
            PrivateKeyKeyringRepo::new(&issuer).expect("existing isolated fixture provider"),
            "lidp".to_owned(),
        ));
        let clients = DbClientRepo::new(idp_engine.client(), keys);
        for client_id in ["idp-evaluator", "untrusted-evaluator"] {
            let mut registration = clients
                .find_client_by_client_id(client_id)
                .await
                .expect("fixture Client lookup")
                .expect("existing evaluator relationship");
            registration.token_endpoint_auth_method = TokenEndpointAuthMethod::ClientSecretBasic;
            clients
                .update_client(registration)
                .await
                .expect("fixture matches evaluator Client authentication method");
        }
        let evaluator_subject = clients
            .find_client_by_client_id("idp-evaluator")
            .await
            .expect("evaluator subject lookup")
            .expect("evaluator Client")
            .id;
        let applications = DbApplicationRepo::new(idp_engine.client());
        let application = applications
            .find_by_uri("https://example.test/unified")
            .await
            .expect("fixture Application lookup")
            .expect("Application")
            .id;
        let other_application = applications
            .find_by_uri("https://example.test/other")
            .await
            .expect("other Application lookup")
            .expect("other Application")
            .id;
        let management_directory = owners.join("device");
        let device_engine = Arc::new(
            Database::open(management_directory.join("devices.redb"))
                .expect("Management owner store"),
        );
        let idp_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("independent IdP listener");
        let idp_address = idp_listener.local_addr().expect("IdP address");
        let management_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("independent Management listener");
        let management_address = management_listener
            .local_addr()
            .expect("Management address");
        let base = format!("https://{idp_address}");
        let idp_base = format!("{base}/idp");
        let management_base = format!("https://{management_address}/device");
        let mut config = idp_server::AppConfig {
            data_dir: owners.join("idp").to_string_lossy().into_owned(),
            service_audience: Some("idp-services".into()),
            management_permissions: Some(idp_server::ManagementPermissionConfig {
                api_base_uri: management_base.clone(),
                client_id: "idp-evaluator".into(),
                client_secret: "idp-evaluator-secret".into(),
            }),
            ..Default::default()
        };
        config.oauth2.issuer = issuer.clone();
        config.oauth2.local_signer = Some(fixture.local_signer.clone());
        config.server.prefix = Some("/idp".into());
        let runtime = idp_server::build_runtime(
            &config,
            idp_engine.client(),
            None,
            Arc::new(
                device_service::device_identity_from_server(&server, secret)
                    .expect("local IdP identity"),
            ),
            server.clone(),
            None,
        )
        .await
        .expect("production independent IdP runtime without local Management policy");
        let control_plane = HostedControlPlane::new_with_services(&idp_base, &idp_base, &issuer)
            .expect("independent Management IdP binding")
            .with_idp_service_client("device-idp", "device-idp-secret", "idp-services")
            .expect("distinct introspection relationship")
            .with_permission_evaluator("idp-evaluator")
            .expect("exact evaluator binding")
            .with_local_devices(Arc::new(device_service::replica::DbDeviceRepo::new(
                device_engine.clone(),
            )))
            .with_pairing_acceptance(runtime.pairing_acceptance());
        let management_router = device_server::build_router(
            device_engine.clone(),
            device_engine.clone(),
            &management_base,
            "/device",
            "storage-service",
            Arc::new(control_plane),
            None,
        )
        .await
        .expect("production Management router");
        let idp_tls = api::LocalTls::open(&TlsConfig {
            root: root.join("idp"),
            address: idp_address,
        })
        .expect("retained IdP TLS");
        let management_tls = api::LocalTls::open(&TlsConfig {
            root: root.join("device"),
            address: management_address,
        })
        .expect("retained Management TLS");
        let idp_router = runtime.router();
        let (stop_idp, idp_stopped) = tokio::sync::oneshot::channel::<()>();
        let idp_task = tokio::spawn(async move {
            axum::serve(idp_tls.listener(idp_listener), idp_router)
                .with_graceful_shutdown(async move {
                    let _ = idp_stopped.await;
                })
                .await
                .expect("production IdP HTTPS server");
        });
        let (stop_management, management_stopped) = tokio::sync::oneshot::channel::<()>();
        let management_task = tokio::spawn(async move {
            axum::serve(
                management_tls.listener(management_listener),
                management_router,
            )
            .with_graceful_shutdown(async move {
                let _ = management_stopped.await;
            })
            .await
            .expect("production Management HTTPS server");
        });
        let client = api::LocalTls::verification_client().expect("production native HTTPS client");
        let bearer = user_token(&client, &base, "administrator", "idp-services").await;
        let applications_url = format!("{idp_base}/applications");
        assert_eq!(
            client
                .get(&applications_url)
                .bearer_auth(&bearer)
                .send()
                .await
                .expect("real policy denial")
                .status(),
            StatusCode::FORBIDDEN
        );
        let policy = ManagementService::new(
            DbPermissionRepo::new(device_engine.client()),
            DbRoleRepo::new(device_engine.client()),
        );
        let role = policy
            .create_role(Id::nil(), "independent application reader", None)
            .await
            .expect("explicit installation role");
        let permission = policy
            .create_permission(
                Id::nil(),
                IdentityAction::ApplicationsRead.permission(),
                None,
            )
            .await
            .expect("exact installation action");
        policy
            .add_permission_to_role(Id::nil(), role.id, permission.id)
            .await
            .expect("exact action grant");
        policy
            .add_role_to_user(Id::nil(), fixture.administrator, role.id)
            .await
            .expect("exact User grant");
        assert_eq!(
            client
                .get(&applications_url)
                .bearer_auth(&bearer)
                .send()
                .await
                .expect("production IdP runtime through remote Management evaluation")
                .status(),
            StatusCode::OK
        );
        let evaluation = format!("{management_base}/permissions/evaluate");
        let evaluator =
            evaluator_token(&client, &base, "idp-evaluator", "idp-evaluator-secret").await;
        let mut request = PermissionEvaluationRequest {
            request_id: Id::now_v7(),
            subject: PermissionSubject::User {
                id: fixture.administrator,
            },
            action: IdentityAction::ApplicationsRead,
            target: PermissionTarget::Installation {
                resource: IdentityResource::Application { id: None },
            },
        };
        assert!(
            decision(
                &client,
                &evaluation,
                &evaluator,
                evaluator_subject,
                &request
            )
            .await
        );
        request.subject = PermissionSubject::User { id: Id::now_v7() };
        assert!(
            !decision(
                &client,
                &evaluation,
                &evaluator,
                evaluator_subject,
                &request
            )
            .await
        );
        request.subject = PermissionSubject::User {
            id: fixture.administrator,
        };
        request.action = IdentityAction::ApplicationsCreate;
        assert!(
            !decision(
                &client,
                &evaluation,
                &evaluator,
                evaluator_subject,
                &request
            )
            .await
        );
        let unbound = evaluator_token(
            &client,
            &base,
            "untrusted-evaluator",
            "untrusted-evaluator-secret",
        )
        .await;
        assert_eq!(
            client
                .post(&evaluation)
                .bearer_auth(&unbound)
                .json(&request)
                .send()
                .await
                .expect("authenticated unbound evaluator")
                .status(),
            StatusCode::FORBIDDEN
        );
        let app_role = policy
            .create_role(application, "application client reader", None)
            .await
            .expect("Application-scoped role");
        let app_permission = policy
            .create_permission(application, IdentityAction::ClientsRead.permission(), None)
            .await
            .expect("Application-scoped action");
        policy
            .add_permission_to_role(application, app_role.id, app_permission.id)
            .await
            .expect("Application-scoped grant");
        policy
            .add_role_to_user(application, fixture.administrator, app_role.id)
            .await
            .expect("Application-scoped actor");
        request.action = IdentityAction::ClientsRead;
        request.target = PermissionTarget::Application {
            application_id: application,
            resource: IdentityResource::Client {
                client_id: Some("human-client".into()),
            },
        };
        assert!(
            decision(
                &client,
                &evaluation,
                &evaluator,
                evaluator_subject,
                &request
            )
            .await
        );
        request.target = PermissionTarget::Application {
            application_id: other_application,
            resource: IdentityResource::Client {
                client_id: Some("other-client".into()),
            },
        };
        assert!(
            !decision(
                &client,
                &evaluation,
                &evaluator,
                evaluator_subject,
                &request
            )
            .await
        );
        assert_eq!(
            client
                .get(&applications_url)
                .bearer_auth("forged-user-token")
                .send()
                .await
                .expect("invalid User authentication")
                .status(),
            StatusCode::UNAUTHORIZED
        );
        stop_management
            .send(())
            .expect("stop independent Management");
        management_task.await.expect("Management HTTPS shutdown");
        assert_eq!(
            client
                .get(&applications_url)
                .bearer_auth(&bearer)
                .send()
                .await
                .expect("Management outage stays fail closed")
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        stop_idp.send(()).expect("stop independent IdP");
        idp_task.await.expect("IdP HTTPS shutdown");
        server.endpoint().close().await;
        drop(runtime);
    })
    .await
    .expect("bounded production Management permission scenario");
}
