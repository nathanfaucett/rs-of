use std::{fs, path::Path};

use crate::IndependentSetupHost;

#[test]
fn independent_join_binds_a_pinned_source_without_an_administrator_seed() {
    let pin = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    let intent: crate::IndependentCreateIntent = serde_json::from_value(serde_json::json!({"purpose":"join", "operation_nonce":"0123456789abcdef0123456789abcdef", "administrator":"", "node":"Joining Node", "idp":"https://127.0.0.1:3000/idp", "device":"https://127.0.0.1:3001/device", "storage":"https://127.0.0.1:3002/storage", "issuer":"https://existing.installation/idp", "peer":format!("https://127.0.0.1:4001/device#{pin}")})).expect("Join intent");
    intent
        .canonical()
        .expect("Join is not a fresh administrator Create");
}

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
    let payload = serde_json::json!({"purpose":"create", "operation_nonce":"0123456789abcdef0123456789abcdef", "administrator":"operator", "node":"Independent Node", "idp":"https://127.0.0.1:3000", "device":"https://127.0.0.1:3001", "storage":"https://127.0.0.1:3002", "issuer":"https://127.0.0.1:3000", "peer":null, "password":"protected-password"});
    for (authorization, expected) in [
        ("Setup wrong".to_owned(), StatusCode::UNAUTHORIZED),
        (format!("Setup {code}"), StatusCode::ACCEPTED),
    ] {
        let response = host
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/independent/setup/create")
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
                .uri("/independent/setup/intent")
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

#[tokio::test]
async fn independent_normal_fallback_keeps_browser_cors_and_authentication() {
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::post,
    };
    use tower::ServiceExt;
    let root = tempfile::tempdir().expect("isolated host");
    let (host, _commands) = prepare(&root.path().join("idp"), "idp", false)
        .expect("prepare")
        .expect("fresh")
        .into_create(tokio_util::sync::CancellationToken::new())
        .expect("host");
    host.control()
        .publish_serving(
            Router::new().route("/oauth2/token", post(|| async { StatusCode::UNAUTHORIZED })),
        )
        .await
        .expect("normal router");
    for method in ["POST", "OPTIONS"] {
        let response = host
            .router()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/oauth2/token")
                    .header("Origin", "https://localhost:5173")
                    .header("Access-Control-Request-Method", "POST")
                    .header(
                        "Access-Control-Request-Headers",
                        "authorization,content-type",
                    )
                    .body(Body::empty())
                    .expect("browser request"),
            )
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            if method == "POST" {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::OK
            }
        );
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .expect("normal browser CORS"),
            "https://localhost:5173"
        );
    }
}

#[tokio::test]
async fn delayed_forged_challenge_does_not_block_cancel_or_commit_after_cancel() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tower::ServiceExt;
    let root = tempfile::tempdir().expect("isolated host");
    let setup = prepare(&root.path().join("device"), "device", false)
        .expect("prepare")
        .expect("fresh");
    let admission = fs::read_to_string(setup.admission_path()).expect("protected admission");
    let code = admission.lines().nth(1).expect("code");
    let (host, mut commands) = setup
        .into_create(tokio_util::sync::CancellationToken::new())
        .expect("host");
    let intent: crate::IndependentCreateIntent = serde_json::from_value(serde_json::json!({"purpose":"create", "operation_nonce":"0123456789abcdef0123456789abcdef", "administrator":"operator", "node":"Independent Node", "idp":"https://127.0.0.1:3001", "device":"https://127.0.0.1:3000", "storage":"https://127.0.0.1:3002", "issuer":"https://127.0.0.1:3001", "peer":null})).expect("operation");
    let response = host
        .router()
        .oneshot(
            Request::post("/independent/setup/create")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Setup {code}"))
                .body(Body::from(serde_json::to_vec(&intent).expect("operation")))
                .expect("authorized Create"),
        )
        .await
        .expect("acceptance");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let command = commands.recv().await.expect("host command");
    let registration = serde_json::json!({"client_id":"00000000-0000-4000-8000-000000000001"});
    let started = Arc::new(tokio::sync::Notify::new());
    let observed = started.clone();
    let (release, blocked) = std::sync::mpsc::channel();
    let blocked = Mutex::new(blocked);
    host.control()
        .set_public(
            crate::IndependentOwnerPublic {
                operation: intent.clone(),
                owner: "device".into(),
                clients: vec![registration.clone()],
                facts: serde_json::json!({}),
                registered: vec![],
            },
            Arc::new(move |_| {
                observed.notify_one();
                blocked
                    .lock()
                    .expect("loader gate")
                    .recv_timeout(Duration::from_secs(5))
                    .expect("bounded loader release");
                Ok("b".repeat(64))
            }),
        )
        .await
        .expect("public evidence");
    let forged = serde_json::json!({"operation":intent,"recipient":"device","registration":registration,"challenge":"a".repeat(64)});
    let callback = tokio::spawn(
        host.router().oneshot(
            Request::post("/independent/setup/independent/challenge")
                .header("Content-Type", "application/json")
                .body(Body::from(forged.to_string()))
                .expect("forged callback"),
        ),
    );
    started.notified().await;
    let cancellation = tokio::time::timeout(
        Duration::from_millis(250),
        host.router().oneshot(
            Request::post("/independent/setup/cancel")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Setup {code}"))
                .body(Body::from(serde_json::to_vec(&intent).expect("operation")))
                .expect("authorized cancel"),
        ),
    )
    .await;
    release.send(()).expect("release delayed provider");
    assert_eq!(
        cancellation
            .expect("forged callback must not hold lifecycle gate")
            .expect("cancel response")
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(command.cancellation.is_cancelled());
    let status = host
        .router()
        .oneshot(
            Request::get("/independent/setup/status")
                .body(Body::empty())
                .expect("status"),
        )
        .await
        .expect("status response");
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(
        callback
            .await
            .expect("callback task")
            .expect("callback response")
            .status(),
        StatusCode::FORBIDDEN
    );
    assert!(
        !host
            .control()
            .registered("00000000-0000-4000-8000-000000000001")
            .await
    );
    assert!(
        !host
            .control()
            .registration_confirmed(&intent, &registration)
            .expect("no receipt")
    );
}

#[test]
fn setup_scope_accepts_only_single_owner_or_complete_unified_host() {
    use crate::SetupScope;

    for owner in ["idp", "device", "storage"] {
        assert_eq!(
            SetupScope::from_owners(&[owner])
                .expect("independent scope")
                .owners(),
            &[owner]
        );
    }
    assert_eq!(
        SetupScope::from_owners(&["storage", "idp", "device"])
            .expect("unified scope")
            .owners(),
        &["idp", "device", "storage"]
    );
    for invalid in [
        &[][..],
        &["idp", "idp"][..],
        &["idp", "device"][..],
        &["idp", "device", "device"][..],
        &["idp", "device", "storage", "storage"][..],
        &["unified"][..],
        &["IdP"][..],
        &["unknown"][..],
        &["idp", "device", "unknown"][..],
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
    for owner in ["idp", "device", "storage"] {
        let domain = temporary.path().join(owner).join("idp");
        let host = prepare(&domain, owner, false)
            .expect("fresh owner")
            .expect("setup");
        let code = fs::read(host.admission_path()).expect("protected retained code");
        drop(host);
        for other in ["idp", "device", "storage"] {
            if other != owner {
                assert!(
                    prepare(&domain, other, false).is_err(),
                    "retained owner cannot be retagged"
                );
            }
        }
        for invalid in ["unified", "unknown", "idp,device"] {
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
                    .uri("/setup/owner/status")
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
                .uri("/setup/owner/admission")
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
    let state = fs::read_to_string(host.admission_path()).expect("protected device retrieval");
    let code = state.lines().nth(1).expect("private code");
    fs::create_dir(&domain).expect("simulate incomplete external provisioning");
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/setup/owner/status")
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
                .uri("/setup/owner/admission")
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
        prepare(&domain, "device", false).is_err(),
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

#[tokio::test]
async fn independent_hosts_serve_canonical_openapi_without_provisioning() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    let root = tempfile::tempdir().expect("isolated metadata hosts");
    for owner in ["idp", "device", "storage"] {
        for (index, prefix) in [String::new(), format!("/{owner}")].iter().enumerate() {
            let domain = root.path().join(format!("{owner}-{index}"));
            let host = IndependentSetupHost::prepare(
                owner,
                &domain,
                "127.0.0.1:3000".parse().expect("address"),
                "https://localhost:5173",
                prefix,
                false,
            )
            .expect("prepare neutral owner")
            .expect("fresh host");
            let neutral = host.router();
            let (host, _commands) = host
                .into_create(tokio_util::sync::CancellationToken::new())
                .expect("independent worker");
            for router in [neutral, host.router()] {
                let response = router
                    .oneshot(
                        Request::get(format!("{prefix}/openapi.json"))
                            .body(Body::empty())
                            .expect("metadata request"),
                    )
                    .await
                    .expect("metadata response");
                assert_eq!(response.status(), StatusCode::OK);
                let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
                    .await
                    .expect("bounded contract");
                let actual: serde_json::Value =
                    serde_json::from_slice(&bytes).expect("served contract");
                let mut expected: serde_json::Value = serde_json::from_str(match owner {
                    "idp" => include_str!("../../../openapi/idp.json"),
                    "device" => include_str!("../../../openapi/device.json"),
                    "storage" => include_str!("../../../openapi/storage.json"),
                    _ => unreachable!("known owner"),
                })
                .expect("canonical owner contract");
                expected["servers"] =
                    serde_json::json!([{ "url": format!("https://127.0.0.1:3000{prefix}") }]);
                assert_eq!(actual, expected, "{owner} discovery at {prefix}");
                assert!(!domain.exists(), "discovery must not provision owner state");
            }
        }
    }
}
