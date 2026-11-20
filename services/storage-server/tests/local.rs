#![cfg(feature = "network")]

use std::{env, fs, sync::Arc};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use db::adapters::MemoryDatabase;
use of_model::id::Id;
use ofdb_sql::Database as SqlDatabase;
use storage_server::runtime::build_local_runtime;
use tower::ServiceExt;

#[tokio::test]
async fn local_runtime_accepts_shared_and_owner_databases_without_setup_or_mesh() {
    for injected in [true, false] {
        let root = env::temp_dir().join(format!("storage-local-{}", Id::now_v7()));
        let (shared, retained) = if injected {
            let adapter = MemoryDatabase::new().await.expect("shared schema");
            (Some(Arc::new(adapter.database())), Some(adapter.storage()))
        } else {
            (None, None)
        };
        let shared: Option<Arc<SqlDatabase>> = shared;
        let runtime = build_local_runtime(
            &root,
            1355,
            "http://127.0.0.1:1355/idp",
            "http://127.0.0.1:1355/device",
            shared,
            retained,
            None,
        )
        .await
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
        #[cfg(feature = "sync")]
        {
            let response = runtime
                .router()
                .oneshot(
                    Request::builder()
                        .uri("/storage/sync/ws")
                        .body(Body::empty())
                        .expect("WS route request"),
                )
                .await
                .expect("WS route response");
            assert_ne!(response.status(), StatusCode::NOT_FOUND);
        }
        assert_eq!(root.join("unified.redb").exists(), !injected);
        assert!(!root.join("storage-local.peer").exists());
        assert!(!root.join("databases").exists());
        assert!(!root.join("filesystems").exists());
        assert!(!root.join("storage.setup-host").exists());
        assert!(runtime.data_handler().is_none());
        runtime.shutdown().await.expect("stop runtime");
        if root.exists() {
            fs::remove_dir_all(root).expect("remove local data");
        }
    }
}
