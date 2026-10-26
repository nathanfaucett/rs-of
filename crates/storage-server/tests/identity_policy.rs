#![cfg(feature = "network")]

use std::{
    future::{Future, poll_fn},
    pin::pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
};

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header::AUTHORIZATION},
};
use device_service::DeviceState;
use idp_model::{contract::IntrospectionResponse, model::Id};
use model::contract::{
    AuthorizationDetail, PrincipalType, StandardClaims, StorageAuthorizationAction,
    StorageAuthorizationDetail, TokenType, TokenUse,
};
use storage_server::{
    DeviceReplicationPolicy, IdpClient, IdpIntrospectionError, LocalReplicationAuthorizer,
    RouterState, resource_router,
};
use storage_service::DatabaseRuntime;
use tokio::sync::Semaphore;
use tower::ServiceExt;

async fn request_status(client: IdpClient, method: Method) -> StatusCode {
    let app = resource_router(
        RouterState::new("http://storage.local").with_idp_client(client),
        None,
        None,
    );
    app.oneshot(
        Request::builder()
            .method(method)
            .uri("/storage/filesystems")
            .header(AUTHORIZATION, "Bearer user-token")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .expect("build storage request"),
    )
    .await
    .expect("storage request must complete")
    .status()
}

#[tokio::test]
async fn identity_failure_returns_without_waiting_for_device_policy() {
    for (identity_error, expected_status) in [
        (
            IdpIntrospectionError::InvalidToken,
            StatusCode::UNAUTHORIZED,
        ),
        (
            IdpIntrospectionError::ServiceUnavailable,
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let gate = Arc::new(Semaphore::new(0));
        let policy = LocalReplicationAuthorizer::new({
            let gate = gate.clone();
            move |_| {
                let gate = gate.clone();
                async move {
                    gate.acquire()
                        .await
                        .expect("release policy lookup")
                        .forget();
                    Err("policy unavailable".to_owned())
                }
            }
        });
        let client = IdpClient::local(move |_| async move { Err(identity_error) })
            .with_local_device_policy(iroh::SecretKey::generate().public(), policy);

        let mut introspection = pin!(client.introspect("invalid-token"));
        let observed = poll_fn(|cx| Poll::Ready(introspection.as_mut().poll(cx))).await;
        let identity_ready = observed.is_ready();
        let result = match observed {
            Poll::Ready(result) => result,
            Poll::Pending => {
                gate.add_permits(1);
                introspection.await
            }
        };
        assert_eq!(result.err(), Some(identity_error));

        let mut request = pin!(request_status(client.clone(), Method::GET));
        let observed = poll_fn(|cx| Poll::Ready(request.as_mut().poll(cx))).await;
        let http_ready = observed.is_ready();
        let status = match observed {
            Poll::Ready(status) => status,
            Poll::Pending => {
                gate.add_permits(1);
                request.await
            }
        };
        assert_eq!(status, expected_status);
        assert!(
            identity_ready && http_ready,
            "definite {identity_error:?} must not wait for gated policy: \
             introspection_ready={identity_ready}, http_ready={http_ready}"
        );
    }
}

fn identity(owner: &str) -> IntrospectionResponse {
    IntrospectionResponse {
        claims: StandardClaims {
            r#type: TokenType::Bearer,
            r#use: TokenUse::Access,
            exp: i64::MAX,
            iat: 0,
            nbf: 0,
            iss: "https://idp.example".into(),
            aud: "storage:databases".into(),
            client_id: "application-client".into(),
            sub: owner.into(),
            principal_type: PrincipalType::User,
            resource: Some("storage:databases".into()),
            authorization_details: Some(vec![AuthorizationDetail::Storage(
                StorageAuthorizationDetail {
                    actions: vec![StorageAuthorizationAction::Read],
                },
            )]),
            scope: vec![],
        },
        application_id: Id::now_v7().to_string(),
    }
}

fn approved_policy(endpoint: iroh::EndpointId, owner: &str) -> DeviceReplicationPolicy {
    DeviceReplicationPolicy {
        device_id: Id::now_v7(),
        endpoint_id: endpoint,
        owner_subject: owner.into(),
        state: DeviceState::Approved,
        owner_active: true,
        admin_allowed: true,
        resources: vec![],
    }
}

#[tokio::test]
async fn valid_identity_waits_for_current_policy_and_observes_learned_denials() {
    let endpoint = iroh::SecretKey::generate().public();
    let owner = Id::now_v7().to_string();
    let response = identity(&owner);
    let baseline = approved_policy(endpoint, &owner);
    let current = Arc::new(Mutex::new(Ok(baseline.clone())));
    let gate = Arc::new(Semaphore::new(0));
    let policy = LocalReplicationAuthorizer::new({
        let current = current.clone();
        let gate = gate.clone();
        move |_| {
            let current = current.clone();
            let gate = gate.clone();
            async move {
                gate.acquire()
                    .await
                    .expect("release current policy")
                    .forget();
                current.lock().expect("current policy").clone()
            }
        }
    });
    let client = IdpClient::local(move |_| {
        let response = response.clone();
        async move { Ok(response) }
    })
    .with_local_device_policy(endpoint, policy);

    let mut pending = pin!(client.introspect("valid-token"));
    assert!(
        poll_fn(|cx| Poll::Ready(pending.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    gate.add_permits(1);
    assert_eq!(
        pending.await.expect("current approved owner").claims.sub,
        owner
    );

    gate.add_permits(32);
    for denial in 0..7 {
        let mut evidence = baseline.clone();
        match denial {
            0 => evidence.owner_subject = Id::now_v7().to_string(),
            1 => evidence.state = DeviceState::Revoked,
            2 => evidence.owner_active = false,
            3 => evidence.admin_allowed = false,
            4 => evidence.endpoint_id = iroh::SecretKey::generate().public(),
            5 => evidence.device_id = Id::nil(),
            _ => {}
        }
        *current.lock().expect("learn current denial") = if denial == 6 {
            Err("policy unavailable".into())
        } else {
            Ok(evidence)
        };
        assert_eq!(
            client.introspect("valid-token").await.err(),
            Some(IdpIntrospectionError::InvalidToken),
            "denial {denial}"
        );
        assert_eq!(
            request_status(client.clone(), Method::GET).await,
            StatusCode::UNAUTHORIZED
        );
    }
    *current.lock().expect("restore current owner") = Ok(baseline);
    assert_eq!(
        client
            .introspect("valid-token")
            .await
            .expect("restored current owner")
            .claims
            .sub,
        owner
    );
}

#[tokio::test]
async fn missing_bearer_is_denied_without_identity_or_policy_work() {
    let client = IdpClient::local(|_| async { panic!("missing bearer must not inspect identity") })
        .with_local_device_policy(
            iroh::SecretKey::generate().public(),
            LocalReplicationAuthorizer::new(|_| async {
                panic!("missing bearer must not query policy")
            }),
        );
    let response = resource_router(
        RouterState::new("http://storage.local").with_idp_client(client),
        None,
        None,
    )
    .oneshot(
        Request::builder()
            .uri("/storage/databases")
            .body(Body::empty())
            .expect("missing bearer request"),
    )
    .await
    .expect("missing bearer response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn storage_response_rechecks_policy_before_returning_success() {
    let endpoint = iroh::SecretKey::generate().public();
    let owner = Id::now_v7().to_string();
    let response = identity(&owner);
    let current = Arc::new(Mutex::new(approved_policy(endpoint, &owner)));
    let gate = Arc::new(Semaphore::new(0));
    let policy = LocalReplicationAuthorizer::new({
        let current = current.clone();
        let gate = gate.clone();
        let calls = AtomicUsize::new(0);
        move |_| {
            let current = current.clone();
            let gate = gate.clone();
            let response_recheck = calls.fetch_add(1, Ordering::SeqCst) == 3;
            async move {
                if response_recheck {
                    gate.acquire()
                        .await
                        .expect("release response recheck")
                        .forget();
                }
                Ok(current.lock().expect("current response policy").clone())
            }
        }
    });
    let client = IdpClient::local(move |_| {
        let response = response.clone();
        async move { Ok(response) }
    })
    .with_local_device_policy(endpoint, policy);
    let root = std::env::temp_dir().join(format!("storage-response-policy-{}", Id::now_v7()));
    let app = resource_router(
        RouterState::new("http://storage.local").with_idp_client(client),
        Some(Arc::new(
            DatabaseRuntime::new(root.clone()).expect("isolated database catalog"),
        )),
        None,
    );
    let request = || {
        Request::builder()
            .uri("/storage/databases")
            .header(AUTHORIZATION, "Bearer valid-token")
            .body(Body::empty())
            .expect("database list request")
    };
    let response = app
        .clone()
        .oneshot(request())
        .await
        .expect("approved database response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        axum::body::to_bytes(response.into_body(), 1024)
            .await
            .expect("database list body")
            .as_ref(),
        b"[]"
    );

    let mut pending = pin!(app.clone().oneshot(request()));
    assert!(
        poll_fn(|cx| Poll::Ready(pending.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    current
        .lock()
        .expect("learn revocation before response")
        .state = DeviceState::Revoked;
    gate.add_permits(1);
    let response = pending.await.expect("revoked response completes");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(
        axum::body::to_bytes(response.into_body(), 1024)
            .await
            .expect("denied response body")
            .is_empty()
    );
    drop(app);
    if root.exists() {
        std::fs::remove_dir_all(root).expect("remove isolated database catalog");
    }
}
