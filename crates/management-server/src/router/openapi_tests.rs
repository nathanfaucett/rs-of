use axum::{body::Body, http::Request};
use std::sync::Arc;
use tower::ServiceExt;
#[tokio::test]
async fn management_serves_only_permission_contract() {
    let identity = Arc::new(db::Database::in_memory());
    let control =
        Arc::new(device_service::HostedControlPlane::new("https://idp.example").expect("control"));
    let router = crate::build_router(
        identity.client(),
        "https://node.example",
        "/management",
        control,
    )
    .await
    .expect("router");
    let response = router
        .oneshot(
            Request::get("/management/openapi.json")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), http::StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let document: serde_json::Value = serde_json::from_slice(&body).expect("contract");
    assert!(
        document["paths"]
            .as_object()
            .expect("paths")
            .keys()
            .all(|path| !path.starts_with("/device")
                && !path.starts_with("/replication")
                && !path.starts_with("/node-joins"))
    );
}
