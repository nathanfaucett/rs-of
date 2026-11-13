#![cfg(feature = "network")]

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use of_model::db::Database;
use tower::ServiceExt;

#[tokio::test]
async fn local_runtime_accepts_shared_and_owner_databases_without_setup_or_mesh() {
    for injected in [true, false] {
        let root = std::env::temp_dir().join(format!(
            "storage-local-{}",
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
        let runtime = storage_server::build_local_runtime(
            &root,
            1355,
            "http://127.0.0.1:1355/idp",
            "http://127.0.0.1:1355/device",
            shared,
            None,
        )
        .expect("local runtime");
        let response = runtime
            .router()
            .oneshot(
                Request::builder()
                    .uri("/storage/health")
                    .body(Body::empty())
                    .expect("health request"),
            )
            .await
            .expect("health response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(root.join("storage.redb").exists(), !injected);
        assert!(root.join("storage-local.peer").is_file());
        assert!(!root.join("storage.setup-host").exists());
        assert!(runtime.data_handler().is_none());
        runtime.shutdown().await.expect("stop runtime");
        std::fs::remove_dir_all(root).expect("remove local data");
    }
}
