#[test]
fn hosted_node_join_contract_is_owned_by_management() {
    let document = management_server::node_join_openapi();
    for path in [
        "/management/node-joins/acceptance",
        "/management/node-joins/pending",
        "/management/node-joins/{id}/approve",
        "/management/node-joins/{id}/reject",
    ] {
        assert!(document.paths.paths.contains_key(path), "missing {path}");
    }
}

#[tokio::test]
async fn service_builder_mounts_hosted_join_routes_at_its_configured_prefix() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use std::sync::Arc;
    use tower::ServiceExt;

    let engine = Arc::new(db::Database::in_memory());
    management_service::replica::up(&engine)
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
    for prefix in ["", "/management", "/custom/management"] {
        let router = management_server::build_router(
            engine.client(),
            engine.clone(),
            "https://management.example",
            prefix,
            "storage",
            control.clone(),
            None,
        )
        .await
        .expect("complete Management service router");
        for (method, path, body) in [
            ("GET", "/node-joins/acceptance", ""),
            ("PUT", "/node-joins/acceptance", r#"{"accepting":true}"#),
            ("GET", "/node-joins/pending", ""),
            (
                "POST",
                "/node-joins/01890f13-4000-7000-8000-000000000001/approve",
                "",
            ),
            (
                "POST",
                "/node-joins/01890f13-4000-7000-8000-000000000001/reject",
                "",
            ),
        ] {
            let request = Request::builder()
                .method(method)
                .uri(format!("{prefix}{path}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("owner request");
            let response = router
                .clone()
                .oneshot(request)
                .await
                .expect("owner response");
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {prefix}{path}"
            );
        }
        let response = router
            .oneshot(
                Request::get(format!("{prefix}/node-joins/acceptance"))
                    .header("authorization", "Bearer unavailable")
                    .body(Body::empty())
                    .expect("configured source request"),
            )
            .await
            .expect("configured source response");
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "missing hosted source fails closed"
        );
    }
}
