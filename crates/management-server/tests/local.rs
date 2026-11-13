use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use of_model::db::Database;
use tower::ServiceExt;

#[tokio::test]
async fn local_constructor_accepts_shared_and_owner_databases_without_setup() {
    for injected in [true, false] {
        let root = std::env::temp_dir().join(format!(
            "management-local-{}",
            of_model::idp::model::Id::now_v7()
        ));
        let shared: Option<Arc<dyn Database>> = if injected {
            Some(Arc::new(
                embedded_db::MemoryDatabase::new()
                    .await
                    .expect("shared schema"),
            ))
        } else {
            None
        };
        let router = management_server::build_local_router(
            &root,
            1355,
            "http://127.0.0.1:1355/idp",
            "http://127.0.0.1:1355/storage",
            shared,
            None,
        )
        .await
        .expect("local constructor");
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/management/health")
                    .body(Body::empty())
                    .expect("health request"),
            )
            .await
            .expect("health response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(root.join("management.redb").exists(), !injected);
        assert!(!root.join("management.setup-host").exists());
        if root.exists() {
            std::fs::remove_dir_all(root).expect("remove owner data");
        }
    }
}
