use std::{fs, path::Path};

use crate::IndependentSetupHost;

fn prepare(
    root: &Path,
    owner: &str,
    configured: bool,
) -> std::io::Result<Option<IndependentSetupHost>> {
    IndependentSetupHost::prepare(
        owner,
        root,
        "127.0.0.1:3000".parse().expect("address"),
        "https://localhost:5173",
        "",
        configured,
    )
}

#[tokio::test]
async fn independent_create_requires_owner_code_and_retains_exact_intent() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let root = tempfile::tempdir().expect("isolated host");
    let setup = prepare(&root.path().join("idp"), "idp", false)
        .expect("prepare")
        .expect("fresh");
    let code = fs::read_to_string(setup.admission_path()).expect("protected code");
    let code = code.lines().nth(1).expect("capability");
    let (host, mut commands) = setup
        .into_create(tokio_util::sync::CancellationToken::new())
        .expect("host");
    let payload = serde_json::json!({"purpose":"create", "operation_nonce":"0123456789abcdef0123456789abcdef", "administrator":"operator", "node":"Independent Node", "idp":"https://127.0.0.1:3000", "management":"https://127.0.0.1:3001", "storage":"https://127.0.0.1:3002", "issuer":"https://127.0.0.1:3000", "peer":null, "password":"protected-password"});
    for (authorization, expected) in [
        ("Setup wrong".to_owned(), StatusCode::UNAUTHORIZED),
        (format!("Setup {code}"), StatusCode::ACCEPTED),
    ] {
        let response = host
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/setup/create")
                    .header("Content-Type", "application/json")
                    .header("Authorization", authorization)
                    .body(Body::from(payload.to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), expected);
    }
    let accepted = commands.recv().await.expect("host-owned command");
    drop(accepted);
    let response = host
        .router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/setup/intent")
                .header("Authorization", format!("Setup {code}"))
                .body(Body::empty())
                .expect("recovery"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 4096)
        .await
        .expect("bounded recovery");
    let recovered: serde_json::Value = serde_json::from_slice(&bytes).expect("recovered intent");
    assert_eq!(recovered["operation_nonce"], payload["operation_nonce"]);
    assert!(recovered.get("password").is_none());
    assert!(
        !String::from_utf8(bytes.to_vec())
            .expect("JSON")
            .contains(code)
    );
}

#[test]
fn setup_scope_accepts_only_single_owner_or_complete_unified_host() {
    use crate::SetupScope;

    for owner in ["idp", "management", "storage"] {
        assert_eq!(
            SetupScope::from_owners(&[owner])
                .expect("independent scope")
                .owners(),
            &[owner]
        );
    }
    assert_eq!(
        SetupScope::from_owners(&["storage", "idp", "management"])
            .expect("unified scope")
            .owners(),
        &["idp", "management", "storage"]
    );
    for invalid in [
        &[][..],
        &["idp", "idp"][..],
        &["idp", "management"][..],
        &["idp", "management", "management"][..],
        &["idp", "management", "storage", "storage"][..],
        &["unified"][..],
        &["IdP"][..],
        &["unknown"][..],
        &["idp", "management", "unknown"][..],
    ] {
        assert!(
            SetupScope::from_owners(invalid).is_err(),
            "invalid scope must refuse"
        );
    }
}

#[tokio::test]
async fn owner_scope_is_retained_not_inferred_from_origin_or_path() {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    let temporary = tempfile::tempdir().expect("isolated roots");
    for owner in ["idp", "management", "storage"] {
        let domain = temporary.path().join(owner).join("idp");
        let host = prepare(&domain, owner, false)
            .expect("fresh owner")
            .expect("setup");
        let code = fs::read(host.admission_path()).expect("protected retained code");
        drop(host);
        for other in ["idp", "management", "storage"] {
            if other != owner {
                assert!(
                    prepare(&domain, other, false).is_err(),
                    "retained owner cannot be retagged"
                );
            }
        }
        for invalid in ["unified", "unknown", "idp,management"] {
            assert!(prepare(&domain, invalid, false).is_err());
        }
        let host = prepare(&domain, owner, false)
            .expect("unchanged reopen")
            .expect("setup");
        assert_eq!(
            fs::read(host.admission_path()).expect("unchanged code"),
            code
        );
        let response = host
            .router()
            .oneshot(
                Request::builder()
                    .uri("/setup/status")
                    .body(Body::empty())
                    .expect("status request"),
            )
            .await
            .expect("public status");
        let body = to_bytes(response.into_body(), 1024)
            .await
            .expect("bounded status body");
        let status: serde_json::Value = serde_json::from_slice(&body).expect("status JSON");
        assert_eq!(
            status["owners"],
            serde_json::json!([owner]),
            "same origin, route and idp-named root are not shared authority"
        );
    }
    let existing = temporary.path().join("existing");
    fs::create_dir(&existing).expect("existing domain root");
    assert!(
        prepare(&existing, "unknown", false).is_err(),
        "unsupported identity is not accepted even on existing roots"
    );
}

#[tokio::test]
async fn exact_https_ui_preflight_allows_explicit_authorization_without_ambient_credentials() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let temporary = tempfile::tempdir().expect("isolated roots");
    let host = prepare(&temporary.path().join("idp"), "idp", false)
        .expect("fresh startup")
        .expect("setup");
    let response = host
        .router()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/setup/admission")
                .header("Origin", "https://localhost:5173")
                .header("Access-Control-Request-Method", "POST")
                .header(
                    "Access-Control-Request-Headers",
                    "authorization,content-type",
                )
                .header("Access-Control-Request-Private-Network", "true")
                .body(Body::empty())
                .expect("preflight"),
        )
        .await
        .expect("preflight response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-private-network")
            .expect("explicit private-network preflight"),
        "true"
    );
    assert!(
        !response
            .headers()
            .contains_key("access-control-allow-credentials")
    );
}

#[tokio::test]
async fn live_awaiting_choice_status_refuses_partial_domain_state() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let temporary = tempfile::tempdir().expect("isolated roots");
    let domain = temporary.path().join("idp");
    let host = prepare(&domain, "idp", false)
        .expect("fresh startup")
        .expect("setup");
    let router = host.router();
    let state = fs::read_to_string(host.admission_path()).expect("protected management retrieval");
    let code = state.lines().nth(1).expect("private code");
    fs::create_dir(&domain).expect("simulate incomplete external provisioning");
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/setup/status")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("status");
    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "domain presence contradicts awaiting choice; never report fresh permission"
    );
    let body = to_bytes(response.into_body(), 1024)
        .await
        .expect("bounded unavailable status");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).expect("unavailable JSON"),
        serde_json::json!({"stage":"installation", "state":"unavailable", "ready":false, "owners":["idp"]})
    );
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/setup/admission")
                .header("Authorization", format!("Setup {code}"))
                .body(Body::empty())
                .expect("admission request"),
        )
        .await
        .expect("admission response");
    assert_eq!(
        response.status(),
        StatusCode::NO_CONTENT,
        "capability verification is not a domain readiness gate or mutation"
    );
    assert!(domain.exists(), "partial domain state is preserved");
}

#[test]
fn retained_owner_setup_is_not_new_admission_permission_after_loss_or_domain_mutation() {
    let temporary = tempfile::tempdir().expect("isolated roots");
    let domain = temporary.path().join("idp");
    let host = prepare(&domain, "idp", false)
        .expect("fresh startup")
        .expect("setup");
    let code = fs::read(host.admission_path()).expect("retained protected admission");
    assert!(
        prepare(&domain, "idp", false).is_err(),
        "exclusive lifecycle lock"
    );
    drop(host);
    let reopened = prepare(&domain, "idp", false)
        .expect("exact reopen")
        .expect("setup");
    assert_eq!(
        fs::read(reopened.admission_path()).expect("same code"),
        code
    );
    let admission_path = reopened.admission_path();
    let ca_path = reopened.tls().ca_path();
    drop(reopened);
    assert!(
        prepare(&domain, "management", false).is_err(),
        "owner cannot adopt another owner root"
    );
    assert!(
        prepare(&domain, "idp", true).is_err(),
        "configured identity is not fresh setup"
    );
    fs::remove_file(ca_path.parent().expect("TLS directory").join("server.key"))
        .expect("lost TLS key");
    assert!(
        prepare(&domain, "idp", false).is_err(),
        "missing private TLS cannot regenerate"
    );
    assert_eq!(
        fs::read(&admission_path).expect("preserved admission"),
        code
    );
    fs::remove_dir_all(admission_path.parent().expect("admission root")).expect("lost admission");
    assert!(
        prepare(&domain, "idp", false).is_err(),
        "missing admission subroot is not fresh permission"
    );
    assert!(!admission_path.exists());
    fs::create_dir(&domain).expect("partial provisioning root");
    assert!(
        prepare(&domain, "idp", false).is_err(),
        "partial provisioning cannot report awaiting choice"
    );
    assert!(domain.exists(), "partial state is preserved");
}
