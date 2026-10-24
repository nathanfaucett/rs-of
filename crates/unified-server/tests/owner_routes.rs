#![cfg(unix)]

use std::sync::Arc;

use axum::http::StatusCode;
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
    let host = unified_server::BootstrapConfig::create_http(
        &root,
        unified_server::BootstrapPurpose::AwaitingChoice,
        None,
        "127.0.0.1:3000".parse().expect("HTTP backend"),
        "https://unified.localhost:1355".into(),
    )
    .expect("neutral host");
    let worker = unified_server::SetupHost::create_http(host)
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
