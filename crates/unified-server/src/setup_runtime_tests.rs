use api::SetupAdmission;
use axum::http::{HeaderMap, StatusCode};
use idp_server::hosted_setup::CreateIntent;
use std::{fs, sync::Arc};
use tokio_util::sync::CancellationToken;
#[tokio::test]
async fn owner_routes_stay_mounted_when_runtime_becomes_available_or_unavailable() {
    use axum::{
        Router,
        body::Body,
        extract::Path,
        http::Request,
        routing::{delete, get},
    };
    use idp_model::model::Id;
    use tower::ServiceExt;

    let root = std::env::temp_dir().join(format!(
        "stable-owner-routes-{}",
        idp_model::model::Id::now_v7()
    ));
    let host = crate::BootstrapConfig::create_http(
        &root,
        crate::BootstrapPurpose::AwaitingChoice,
        None,
        "127.0.0.1:3000".parse().expect("HTTP backend"),
        "https://unified.localhost:1355".into(),
    )
    .expect("neutral host");
    let worker = crate::SetupHost::create_http(host)
        .expect("HTTP setup")
        .into_worker(CancellationToken::new());
    let router = worker.router().expect("stable router");
    let owner = Router::new()
        .route(
            "/idp/.well-known/openid-configuration",
            get(|| async { "discovery" }),
        )
        .route(
            "/idp/users/{user_id}",
            get(|Path(_id): Path<Id>| async { StatusCode::UNAUTHORIZED }),
        )
        .route(
            "/idp/users/{user_id}/consents/{consent_id}",
            delete(|Path((_user, _consent)): Path<(Id, Id)>| async { StatusCode::NO_CONTENT }),
        )
        .route("/storage", get(|| async { StatusCode::OK }));
    let user = Id::now_v7();
    let consent = Id::now_v7();
    for available in [false, true, false] {
        if available {
            worker
                .control()
                .publish(idp_server::HostedSetupReady {
                    router: owner.clone(),
                    readiness: Arc::new(|| Box::pin(async { Ok(true) })),
                    login_client: idp_server::hosted_setup::LoginClient {
                        client_id: "fixture".into(),
                        scopes: vec![],
                    },
                })
                .await;
        } else {
            worker.control().withdraw().await;
        }
        for (method, path, ready_status) in [
            (
                "GET",
                "/idp/.well-known/openid-configuration".to_owned(),
                StatusCode::OK,
            ),
            (
                "GET",
                format!("/idp/users/{user}"),
                StatusCode::UNAUTHORIZED,
            ),
            (
                "DELETE",
                format!("/idp/users/{user}/consents/{consent}"),
                StatusCode::NO_CONTENT,
            ),
            ("GET", "/storage".to_owned(), StatusCode::OK),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .expect("owner request"),
                )
                .await
                .expect("owner response");
            assert_eq!(
                response.status(),
                if available {
                    ready_status
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            );
        }
        for path in [
            "/idp/version",
            "/management/openapi.json",
            "/storage/openapi.json",
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::get(path)
                        .body(Body::empty())
                        .expect("metadata request"),
                )
                .await
                .expect("metadata response");
            assert_eq!(response.status(), StatusCode::OK);
        }
        for path in ["/idp/nonexistent", "/storage/"] {
            let response = router
                .clone()
                .oneshot(
                    Request::get(path)
                        .body(Body::empty())
                        .expect("unknown request"),
                )
                .await
                .expect("unknown response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }
    drop(worker);
    std::fs::remove_dir_all(root).expect("isolated cleanup");
}

pub(crate) async fn status_tracks_revocation_in_existing_runtime(
    mut runtime: crate::UnifiedRuntime,
    local: crate::LocalProvisioner,
    _tls: &crate::LocalTls,
    root: &std::path::Path,
    role: idp_model::model::Id,
) {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    async fn observe(
        router: &axum::Router,
        expected: StatusCode,
        ready: bool,
    ) -> serde_json::Value {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/idp/setup/status")
                    .body(Body::empty())
                    .expect("status request"),
            )
            .await
            .expect("production status router");
        assert_eq!(response.status(), expected);
        let body: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), 2048)
                .await
                .expect("status body"),
        )
        .expect("status JSON");
        assert_eq!(body["ready"], ready);
        if !ready {
            assert!(
                body.get("login_client").is_none(),
                "unready status must not advertise login readiness"
            );
        }
        body
    }

    let policy = runtime.full_node_test_management();
    let service = management_service::ManagementService::new(
        management_service::replica::DbPermissionRepo::new(policy.clone()),
        management_service::replica::DbRoleRepo::new(policy),
    );
    let application = idp_model::contract::INSTALLATION_POLICY_ID;
    let permission = service
        .list_permissions(application, 0, 128)
        .await
        .expect("real public permission inventory")
        .into_iter()
        .find(|permission| {
            permission.name == idp_model::contract::IdentityAction::IdpSecuritySync.permission()
        })
        .expect("required IdP context grant")
        .id;
    let mut host =
        crate::BootstrapConfig::create(root, crate::BootstrapPurpose::AwaitingChoice, None)
            .expect("isolated status host metadata");
    host.begin_setup().expect("host setup history");
    let admission =
        Arc::new(SetupAdmission::open(&root.join("setup-admission")).expect("protected admission"));
    let retained =
        fs::read_to_string(root.join("setup-admission/state")).expect("protected retrieval");
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        format!("Setup {}", retained.lines().nth(1).expect("code"))
            .parse()
            .expect("setup header"),
    );
    let intent = CreateIntent {
        operation_nonce: "0123456789abcdef0123456789abcdef".into(),
        administrator: "operator".into(),
        node: "HTTPS source".into(),
    }
    .canonical()
    .expect("completed result intent");
    admission
        .bind(&headers, &intent)
        .expect("retained exact setup result");
    admission
        .retire(&headers, &intent)
        .expect("retired setup admission");
    let (control, _commands) = idp_server::HostedSetupControl::new(
        admission,
        "unfinished",
        vec![],
        CancellationToken::new(),
    );
    let router = control.router(idp_server::build_setup_router(
        control.clone(),
        "https://localhost/idp",
        "/idp",
    ));
    runtime.trace_bootstrap_http_stages();
    runtime
        .start_background_tasks()
        .await
        .expect("start ready owners");
    let (protocol, owner) = runtime.readiness();
    let login = local.login_client().expect("public login");
    control
        .publish(idp_server::HostedSetupReady {
            router: runtime.router(),
            readiness: Arc::new(move || {
                let protocol = protocol.clone();
                let owner = owner.clone();
                Box::pin(async move { protocol.local_node_public_ready(owner.as_ref()).await })
            }),
            login_client: idp_server::hosted_setup::LoginClient {
                client_id: login.client_id.expect("ready Client"),
                scopes: login.allowed_scopes,
            },
        })
        .await;
    local.mark_complete().expect("real source owner completion");
    let completed = observe(&router, StatusCode::OK, true).await;

    service
        .remove_permission_from_role(application, role, permission)
        .await
        .expect("learn local context-grant revocation through owner facade");
    observe(&router, StatusCode::SERVICE_UNAVAILABLE, false).await;
    let normal = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/management/replication/devices/untrusted/selections")
                .body(Body::empty())
                .expect("normal protected request"),
        )
        .await
        .expect("normal owner router");
    assert_eq!(
        normal.status(),
        StatusCode::UNAUTHORIZED,
        "readiness failure is not a blanket route gate"
    );
    service
        .add_permission_to_role(application, role, permission)
        .await
        .expect("authorized local grant restoration");
    let restored = observe(&router, StatusCode::OK, true).await;
    assert_eq!(
        restored["login_client"], completed["login_client"],
        "status recomputes readiness without replacing the completed identity"
    );
    control.withdraw().await;
    runtime
        .shutdown()
        .await
        .expect("coordinated real owner cleanup");
    observe(&router, StatusCode::SERVICE_UNAVAILABLE, false).await;
    drop(router);
    drop(control);
    fs::remove_dir_all(root).expect("isolated status metadata cleanup");
}
