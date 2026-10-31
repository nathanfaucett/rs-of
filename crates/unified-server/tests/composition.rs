use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use unified_server::{SetupHost, UnifiedConfig};

#[tokio::test]
async fn owner_surfaces_share_one_endpoint_and_exactly_one_prefix() {
    let root = std::env::temp_dir().join(format!(
        "owner-composition-{}",
        idp_model::model::Id::now_v7()
    ));
    let config = UnifiedConfig::local(&root, "127.0.0.1:37443".parse().expect("loopback"));
    let host = SetupHost::prepare(config).expect("composed owner setup");
    let router = host.router();
    for owner in ["idp", "device", "storage"] {
        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/{owner}/setup/owner/status"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK, "{owner}");
        for path in [
            format!("/{owner}/{owner}/setup/owner/status"),
            format!("/{owner}/setup/status"),
            format!("/{owner}/setup/independent/create"),
        ] {
            let response = router
                .clone()
                .oneshot(Request::get(&path).body(Body::empty()).expect("request"))
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
        assert!(
            host.admission_path(owner)
                .expect("owner admission")
                .is_file()
        );
    }
    for path in [
        "/setup/owner/status",
        "/management/setup/owner/status",
        "/setup/status",
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).expect("request"))
                .await
                .expect("response")
                .status(),
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    let supplied = unified_server::compose_router(
        axum::Router::new().route("/idp/health", axum::routing::get(|| async { "idp" })),
        axum::Router::new().route(
            "/management/health",
            axum::routing::get(|| async { "management" }),
        ),
        axum::Router::new().route("/device/health", axum::routing::get(|| async { "device" })),
        axum::Router::new().route(
            "/storage/health",
            axum::routing::get(|| async { "storage" }),
        ),
    );
    for owner in ["idp", "management", "device", "storage"] {
        for (path, expected) in [
            (format!("/{owner}/health"), StatusCode::OK),
            (format!("/{owner}/{owner}/health"), StatusCode::NOT_FOUND),
        ] {
            assert_eq!(
                supplied
                    .clone()
                    .oneshot(
                        Request::get(path)
                            .body(Body::empty())
                            .expect("supplied owner request")
                    )
                    .await
                    .expect("supplied owner response")
                    .status(),
                expected
            );
        }
    }
    drop(router);
    drop(host);
    let legacy = root.join("previous-installation");
    std::fs::create_dir(&legacy).expect("retained prior host root");
    std::fs::write(legacy.join("host.redb"), b"retained prior installation")
        .expect("previous host evidence");
    let failure = SetupHost::prepare(UnifiedConfig::local(
        &legacy,
        "127.0.0.1:37443".parse().expect("loopback"),
    ))
    .err()
    .expect("previous host must not be reprovisioned");
    assert!(failure.to_string().contains("previous binary"));
    assert_eq!(
        std::fs::read(legacy.join("host.redb")).expect("preserved prior installation"),
        b"retained prior installation"
    );
    let orphan = root.join("orphaned-endpoint");
    std::fs::create_dir_all(orphan.join("endpoint")).expect("orphaned prior endpoint");
    let failure = SetupHost::prepare(UnifiedConfig::local(
        &orphan,
        "127.0.0.1:37443".parse().expect("loopback"),
    ))
    .err()
    .expect("orphaned prior endpoint must not authorize fresh owners");
    assert!(failure.to_string().contains("previous binary"));
    assert_eq!(
        std::fs::read_dir(&orphan).expect("preserved root").count(),
        1
    );
    std::fs::remove_dir_all(root).expect("fixture cleanup");
}
