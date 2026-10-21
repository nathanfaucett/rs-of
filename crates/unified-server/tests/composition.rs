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
