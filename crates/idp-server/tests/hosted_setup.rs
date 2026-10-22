#![cfg(unix)]

use std::fs;

use idp_model::model::Id;
use tokio_util::sync::CancellationToken;
#[test]
fn hosted_setup_contract_is_owned_by_idp() {
    let document = idp_server::setup_openapi();
    for path in [
        "/idp/setup/status",
        "/idp/setup/create",
        "/idp/setup/join",
        "/idp/setup/cancel",
        "/idp/setup/admission",
        "/idp/setup/intent",
    ] {
        assert!(document.paths.paths.contains_key(path), "missing {path}");
    }
}

#[tokio::test]
async fn retained_intent_route_rejects_invalid_tuples_without_disclosure_or_repair() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    let root = std::env::temp_dir().join(format!("intent-readback-{}", Id::now_v7()));
    api::setup_files::create_private_directory(&root, false).expect("private parent");
    let admission = std::sync::Arc::new(
        api::SetupAdmission::open(&root.join("setup-admission")).expect("protected admission"),
    );
    let state = root.join("setup-admission/state");
    let initial = fs::read_to_string(&state).expect("protected retrieval");
    let code = initial.lines().nth(1).expect("host code");
    let (control, _commands) = idp_server::HostedSetupControl::new(
        admission,
        "awaiting_choice",
        vec![],
        CancellationToken::new(),
    );
    let router = idp_server::build_setup_router(control, "https://localhost/idp", "/idp");
    for retained in [
        "not-json",
        r#"["join",["idp","management","storage"],"0123456789abcdef0123456789abcdef","operator","Test Node"]"#,
        r#"["create",["idp","storage","management"],"0123456789abcdef0123456789abcdef","operator","Test Node"]"#,
        r#"["create",["idp"],"0123456789abcdef0123456789abcdef","operator","Test Node"]"#,
        r#"["create",["idp","management","storage"],"invalid-nonce","operator","Test Node"]"#,
        r#"["create",["idp","management","storage"],"0123456789abcdef0123456789abcdef"," operator","Test Node"]"#,
        r#"["create", ["idp","management","storage"],"0123456789abcdef0123456789abcdef","operator","Test Node"]"#,
        r#"["create",["idp","management","storage"],"0123456789abcdef0123456789abcdef","operator","Test Node","password"]"#,
    ] {
        let injected = format!("setup-admission-v1\n{code}\n{retained}");
        fs::write(&state, &injected).expect("inject invalid retained tuple");
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/idp/setup/intent")
                    .header("Authorization", format!("Setup {code}"))
                    .body(Body::empty())
                    .expect("body-free lookup"),
            )
            .await
            .expect("infallible setup router");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            to_bytes(response.into_body(), 1024)
                .await
                .expect("safe error body")
                .is_empty()
        );
        assert_eq!(
            fs::read_to_string(&state).expect("retained fault unchanged"),
            injected
        );
    }
    drop(router);
    fs::remove_dir_all(root).expect("isolated cleanup");
}

#[tokio::test]
async fn service_builder_mounts_setup_status_and_admission_at_its_configured_prefix() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use std::sync::Arc;
    use tower::ServiceExt;

    let root = std::env::temp_dir().join(format!("idp-setup-builder-{}", Id::now_v7()));
    api::setup_files::create_private_directory(&root, false).expect("private parent");
    let admission =
        Arc::new(api::SetupAdmission::open(&root.join("admission")).expect("setup capability"));
    for prefix in ["", "/idp", "/custom/idp"] {
        let (control, _commands) = idp_server::HostedSetupControl::new(
            admission.clone(),
            "awaiting_choice",
            vec![],
            CancellationToken::new(),
        );
        let router = idp_server::build_setup_router(control, "https://idp.example", prefix);
        let response = router
            .clone()
            .oneshot(
                Request::get(format!("{prefix}/setup/status"))
                    .body(Body::empty())
                    .expect("status request"),
            )
            .await
            .expect("status response");
        assert_eq!(response.status(), StatusCode::OK);
        let status: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), 2048)
                .await
                .expect("status body"),
        )
        .expect("public status");
        assert_eq!(
            status,
            serde_json::json!({"stage":"installation", "state":"awaiting_choice", "ready":false, "owners":["idp","management","storage"]})
        );
        for path in ["/setup/admission", "/setup/intent"] {
            let response = router
                .clone()
                .oneshot(
                    Request::post(format!("{prefix}{path}"))
                        .body(Body::empty())
                        .expect("unauthenticated setup request"),
                )
                .await
                .expect("setup denial");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }
    fs::remove_dir_all(root).expect("isolated cleanup");
}
