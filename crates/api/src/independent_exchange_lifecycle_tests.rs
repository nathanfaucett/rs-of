use std::{
    fs,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

use super::{Challenge, OutstandingChallenge};
use crate::{IndependentCreateIntent, IndependentSetupHost};

#[tokio::test]
async fn gated_registrar_allows_status_and_cancel_and_rejects_late_completion() {
    let root = tempfile::tempdir().expect("isolated host");
    let setup = IndependentSetupHost::prepare(
        "idp",
        &root.path().join("idp"),
        "127.0.0.1:3000".parse().expect("address"),
        "https://localhost:5173",
        "",
        false,
    )
    .expect("prepare")
    .expect("fresh host");
    let admission = fs::read_to_string(setup.admission_path()).expect("protected admission");
    let code = admission.lines().nth(1).expect("host capability");
    let (host, mut commands) = setup.into_create(CancellationToken::new()).expect("host");
    let intent = IndependentCreateIntent {
        purpose: "create".into(),
        operation_nonce: "0123456789abcdef0123456789abcdef".into(),
        administrator: "operator".into(),
        node: "Independent Node".into(),
        idp: "https://127.0.0.1:3000".into(),
        device: "https://127.0.0.1:3001".into(),
        storage: "https://127.0.0.1:3002".into(),
        issuer: "https://127.0.0.1:3000".into(),
        peer: None,
    };
    let mut create = serde_json::to_value(&intent).expect("intent");
    create["password"] = serde_json::json!("protected-password");
    let response = host
        .router()
        .oneshot(
            Request::post("/independent/setup/create")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Setup {code}"))
                .body(Body::from(create.to_string()))
                .expect("authorized Create"),
        )
        .await
        .expect("acceptance");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let command = commands.recv().await.expect("host-owned command");
    let control = host.control();
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let observed = started.clone();
    let gate = release.clone();
    control
        .set_registrar(Arc::new(move |_| {
            let observed = observed.clone();
            let gate = gate.clone();
            Box::pin(async move {
                observed.notify_one();
                gate.notified().await;
                Ok(())
            })
        }))
        .await;
    let registration = serde_json::json!({"client_id":"00000000-0000-4000-8000-000000000001"});
    let bound = Challenge {
        operation: intent.clone(),
        recipient: "device".into(),
        registration: registration.clone(),
        challenge: "a".repeat(64),
    };
    control
        .0
        .lifecycle
        .lock()
        .await
        .challenges
        .push(OutstandingChallenge {
            bound: bound.clone(),
            expires: Instant::now() + Duration::from_secs(30),
        });
    let mut request = serde_json::to_value(bound).expect("registration");
    request["registration"]["client_secret"] = serde_json::json!("b".repeat(64));
    let callback = tokio::spawn(
        host.router().oneshot(
            Request::post("/independent/setup/independent/register")
                .header("Content-Type", "application/json")
                .body(Body::from(request.to_string()))
                .expect("exact registration callback"),
        ),
    );
    tokio::time::timeout(Duration::from_secs(2), started.notified())
        .await
        .expect("registrar reached gate");
    let status = tokio::time::timeout(
        Duration::from_millis(250),
        host.router().oneshot(
            Request::get("/independent/setup/status")
                .body(Body::empty())
                .expect("status request"),
        ),
    )
    .await;
    let cancellation = tokio::time::timeout(
        Duration::from_millis(250),
        host.router().oneshot(
            Request::post("/independent/setup/cancel")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Setup {code}"))
                .body(Body::from(serde_json::to_vec(&intent).expect("operation")))
                .expect("authorized exact cancel"),
        ),
    )
    .await;
    release.notify_one();
    let response = tokio::time::timeout(Duration::from_secs(2), callback)
        .await
        .expect("released callback completes")
        .expect("callback task")
        .expect("callback response");
    assert_eq!(
        status
            .expect("status must complete while registrar is gated")
            .expect("status response")
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        cancellation
            .expect("cancel must complete while registrar is gated")
            .expect("cancel response")
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(command.cancellation.is_cancelled());
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(
        !control
            .registration_confirmed(&intent, &registration)
            .expect("no late receipt")
    );
    assert_eq!(
        control
            .finish(&command, Router::new(), &serde_json::json!({}))
            .await
            .expect_err("cancelled command cannot finalize")
            .kind(),
        std::io::ErrorKind::Interrupted
    );
    let admission = host
        .router()
        .oneshot(
            Request::post("/independent/setup/admission")
                .header("Authorization", format!("Setup {code}"))
                .body(Body::empty())
                .expect("admission request"),
        )
        .await
        .expect("admission response");
    assert_eq!(
        admission.status(),
        StatusCode::NO_CONTENT,
        "late completion must not retire admission"
    );
}
