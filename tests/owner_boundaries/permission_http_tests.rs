#[cfg(unix)]
#[path = "permission_http_tests/remote_identity_process_tests.rs"]
mod remote_identity_process_tests;

use std::sync::Arc;

use db::Database;
use device_service::replica::DbDeviceRepo;
use idp_model::contract::{
    IdentityAction, IdentityResource, PermissionEvaluationRequest, PermissionSubject,
    PermissionTarget,
};
use tower::ServiceExt;

use crate::support::identity::{
    expect_status, run_lifecycle_http_checks, run_permission_http_checks, seed_idp,
    seed_permissions, user_token,
};

#[test]
fn normal_permission_http_administration() {
    run_http_test(false);
}

#[test]
fn normal_identity_and_infrastructure_lifecycle() {
    run_http_test(true);
}

fn run_http_test(lifecycle_only: bool) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_stack_size(16 * 1024 * 1024)
                .enable_all()
                .build()
                .expect("build HTTP test runtime")
                .block_on(run_separate_permission_acceptance(lifecycle_only));
        })
        .expect("spawn HTTP acceptance")
        .join()
        .expect("HTTP acceptance did not panic");
}

async fn run_separate_permission_acceptance(lifecycle_only: bool) {
    let root = std::env::temp_dir().join(format!(
        "permission-http-{}",
        idp_model::model::Id::now_v7()
    ));
    let issuer = format!(
        "https://permission-{}.example",
        idp_model::model::Id::now_v7()
    );
    let idp_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind IdP listener");
    let management_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind Management listener");
    let idp_address = idp_listener.local_addr().expect("IdP address");
    let management_address = management_listener
        .local_addr()
        .expect("Management address");
    let idp_base = format!("http://{idp_address}");
    let management_base = format!("http://{management_address}");
    let secret = iroh::SecretKey::generate();
    let mut fixture = seed_idp(
        &root,
        &issuer,
        &secret.public().to_string(),
        &iroh::SecretKey::generate().public().to_string(),
    )
    .await;
    seed_permissions(&root, &mut fixture).await;
    eprintln!("permission HTTP fixtures ready");
    let server = iroh_chain::Server::bind_with_secret_key(
        iroh::endpoint::presets::N0,
        secret.clone(),
        iroh_chain::EndpointIdStore::default(),
    )
    .await
    .expect("bind test endpoint");
    let mut config = idp_server::AppConfig::default();
    config.oauth2.issuer = issuer.clone();
    config.oauth2.local_signer = Some(fixture.local_signer.clone());
    config.service_audience = Some("idp-services".into());
    config.server.prefix = Some("/idp".into());
    let idp_engine =
        Arc::new(Database::open(root.join("idp/idp.redb")).expect("open IdP owner engine"));
    let device_engine = Arc::new(
        Database::open(root.join("device/devices.redb")).expect("open Management owner engine"),
    );
    let runtime = idp_server::build_runtime(
        &config,
        idp_engine.client(),
        None,
        Arc::new(
            device_service::device_identity_from_server(&server, secret.clone())
                .expect("create local identity"),
        ),
        server.clone(),
        Some(management_service::LocalPermissions::new(
            idp_engine.client(),
        )),
    )
    .await
    .expect("build independent IdP runtime");
    let control_plane = device_service::HostedControlPlane::new_with_services(
        &format!("{idp_base}/idp"),
        &format!("{idp_base}/unused-storage"),
        &issuer,
    )
    .expect("normal Management IdP client")
    .with_idp_service_client("management-idp", "management-idp-secret", "idp-services")
    .expect("configure introspection relationship")
    .with_permission_evaluator("idp-evaluator")
    .expect("pin distinct assertion client")
    .with_local_devices(Arc::new(DbDeviceRepo::new(device_engine.clone().client())))
    .with_pairing_acceptance(runtime.pairing_acceptance());
    let control_plane = Arc::new(control_plane);
    let router = management_server::build_router(
        idp_engine.client(),
        &management_base,
        "/management",
        control_plane.clone(),
    )
    .await
    .expect("Management role router")
    .merge(
        device_server::build_router(
            idp_engine.clone().client(),
            device_engine.client(),
            &management_base,
            "/device",
            "storage-service",
            control_plane,
            None,
        )
        .await
        .expect("Device router"),
    );
    let idp_router = runtime.router();
    let mut idp_task = tokio::spawn(async move {
        axum::serve(idp_listener, idp_router)
            .await
            .expect("serve IdP");
    });
    let management_task = tokio::spawn(async move {
        axum::serve(management_listener, router)
            .await
            .expect("serve Management");
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .expect("bounded test client");
    if lifecycle_only {
        run_lifecycle_http_checks(&client, &idp_base, &management_base, &fixture).await;
        let actor = user_token(&client, &idp_base, "administrator", "idp-services").await;
        let unconfigured = idp_server::build_runtime(
            &config,
            idp_engine.client(),
            None,
            Arc::new(
                device_service::device_identity_from_server(&server, secret)
                    .expect("local identity"),
            ),
            server.clone(),
            None,
        )
        .await
        .expect("build without permission relationship");
        let response = unconfigured
            .router()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/idp/applications")
                    .header("authorization", format!("Bearer {actor}"))
                    .body(axum::body::Body::empty())
                    .expect("normal authenticated admin request"),
            )
            .await
            .expect("call unconfigured owner API");
        assert_eq!(response.status(), http::StatusCode::FORBIDDEN);
        drop(unconfigured);
        idp_task.abort();
        management_task.abort();
        let _ = idp_task.await;
        let _ = management_task.await;
        server.endpoint().close().await;
        drop(runtime);
        std::fs::remove_dir_all(root).expect("remove lifecycle fixture data");
        return;
    }
    run_permission_http_checks(
        &client,
        &idp_base,
        &management_base,
        &fixture,
        http::StatusCode::OK,
    )
    .await;
    let admin = user_token(&client, &idp_base, "administrator", "idp-services").await;
    let management_user = user_token(&client, &idp_base, "administrator", "idp-management").await;
    idp_task.abort();
    let _ = idp_task.await;
    let evaluation = PermissionEvaluationRequest {
        request_id: idp_model::model::Id::now_v7(),
        subject: PermissionSubject::User {
            id: fixture.administrator,
        },
        action: IdentityAction::ClientsRead,
        target: PermissionTarget::Application {
            application_id: fixture.application_id,
            resource: IdentityResource::Client {
                client_id: Some("human-client".into()),
            },
        },
    };
    expect_status(
        client
            .post(format!("{management_base}/management/permissions/evaluate"))
            .bearer_auth(&management_user)
            .json(&evaluation)
            .send()
            .await
            .expect("IdP outage request"),
        http::StatusCode::SERVICE_UNAVAILABLE,
    )
    .await;
    let idp_listener = tokio::net::TcpListener::bind(idp_address)
        .await
        .expect("restart IdP listener");
    let idp_router = runtime.router();
    idp_task = tokio::spawn(async move {
        axum::serve(idp_listener, idp_router)
            .await
            .expect("serve restarted IdP");
    });
    management_task.abort();
    let _ = management_task.await;
    expect_status(
        client
            .get(format!("{idp_base}/idp/oauth2/register/human-client"))
            .bearer_auth(&admin)
            .send()
            .await
            .expect("Management listener outage does not revoke local policy"),
        http::StatusCode::OK,
    )
    .await;
    let fresh_user = user_token(&client, &idp_base, "administrator", "idp-services").await;
    let token = expect_status(
        client
            .post(format!("{idp_base}/idp/oauth2/token"))
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", "management-idp"),
                ("client_secret", "management-idp-secret"),
                ("audience", "idp-services"),
                ("scope", "idp.token.validate"),
            ])
            .send()
            .await
            .expect("issue introspection client token without Management"),
        http::StatusCode::OK,
    )
    .await;
    expect_status(
        client
            .post(format!("{idp_base}/idp/oauth2/introspect"))
            .bearer_auth(token["access_token"].as_str().expect("service token"))
            .json(&idp_model::contract::IntrospectionRequest {
                token: fresh_user,
                token_type_hint: None,
            })
            .send()
            .await
            .expect("introspection without Management"),
        http::StatusCode::OK,
    )
    .await;
    idp_task.abort();
    let _ = idp_task.await;
    eprintln!("closing permission HTTP endpoint");
    server.endpoint().close().await;
    eprintln!("permission HTTP endpoint closed");
    drop(runtime);
    std::fs::remove_dir_all(root).expect("remove HTTP fixture data");
}
