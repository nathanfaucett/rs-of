use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::get,
};
use tower::ServiceExt;
use unified_server::compose_router;

async fn assert_response(router: &Router, path: &str, status: StatusCode, expected: &str) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("build composition request"),
        )
        .await
        .expect("call composed router");
    assert_eq!(response.status(), status, "{path}");
    let body = to_bytes(response.into_body(), 1024)
        .await
        .expect("read composition response");
    assert_eq!(body.as_ref(), expected.as_bytes(), "{path}");
}

#[tokio::test]
async fn composition_exposes_only_routes_supplied_by_the_three_services() {
    let idp = Router::new().route("/idp/health", get(|| async { "idp" }));
    let management = Router::new().route("/management/health", get(|| async { "management" }));
    let storage = Router::new().route("/storage/databases", get(|| async { "storage" }));
    let router = compose_router(idp.clone(), management.clone(), storage.clone());

    for (path, expected) in [
        ("/idp/health", "idp"),
        ("/management/health", "management"),
        ("/storage/databases", "storage"),
    ] {
        assert_response(&router, path, StatusCode::OK, expected).await;
    }
    for path in [
        "/idp/setup/status",
        "/management/node-joins/acceptance",
        "/management/node-joins/pending",
        "/idp/idp/health",
        "/management/management/health",
        "/storage/storage/databases",
    ] {
        assert_response(&router, path, StatusCode::NOT_FOUND, "").await;
    }

    let router = compose_router(
        idp.route("/idp/setup/status", get(|| async { "setup" })),
        management.route(
            "/management/node-joins/acceptance",
            get(|| async { "acceptance" }),
        ),
        storage,
    );
    assert_response(&router, "/idp/setup/status", StatusCode::OK, "setup").await;
    assert_response(
        &router,
        "/management/node-joins/acceptance",
        StatusCode::OK,
        "acceptance",
    )
    .await;
}

#[cfg(unix)]
#[tokio::test]
async fn owner_service_builders_supply_hosted_features_to_composition() {
    use std::sync::Arc;

    let root = std::env::temp_dir().join(format!(
        "service-composition-{}",
        idp_model::model::Id::now_v7()
    ));
    api::setup_files::create_private_directory(&root, false).expect("private setup fixture");
    let admission =
        Arc::new(api::SetupAdmission::open(&root.join("admission")).expect("protected admission"));
    let (setup, _commands) = idp_server::HostedSetupControl::new(
        admission,
        "awaiting_choice",
        vec![],
        tokio_util::sync::CancellationToken::new(),
    );
    let management = Arc::new(db::Database::in_memory());
    management_service::replica::up(&management)
        .await
        .expect("Management schema");
    let control = Arc::new(
        management_service::HostedControlPlane::new_with_services(
            "https://idp.example",
            "https://storage.example",
            "https://installation.example",
        )
        .expect("owner interfaces"),
    );
    let router = compose_router(
        idp_server::build_setup_router(setup, "https://idp.example/idp", "/idp"),
        management_server::build_router(
            management,
            "https://management.example",
            "/management",
            "storage",
            control,
            None,
        )
        .await
        .expect("Management router"),
        storage_server::public_metadata_router("https://storage.example/storage", "/storage"),
    );
    for path in [
        "/idp/version",
        "/management/health",
        "/storage/version",
        "/idp/setup/status",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::get(path)
                    .body(Body::empty())
                    .expect("owner request"),
            )
            .await
            .expect("composed owner response");
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }
    let response = router
        .clone()
        .oneshot(
            Request::get("/management/node-joins/acceptance")
                .body(Body::empty())
                .expect("join request"),
        )
        .await
        .expect("composed Management route");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    for path in [
        "/idp/nonexistent",
        "/management/nonexistent",
        "/storage/nonexistent",
        "/idp/idp/setup/status",
    ] {
        assert_response(&router, path, StatusCode::NOT_FOUND, "").await;
    }
    drop(router);
    std::fs::remove_dir_all(root).expect("isolated cleanup");
}
