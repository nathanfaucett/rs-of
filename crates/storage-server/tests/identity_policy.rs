#![cfg(feature = "network")]

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header::AUTHORIZATION},
};
use storage_server::{IdpClient, RouterState, resource_router};
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
    use std::{
        future::{Future, poll_fn},
        pin::pin,
        sync::Arc,
        task::Poll,
    };

    use tokio::sync::Semaphore;

    use storage_server::{IdpIntrospectionError, LocalReplicationAuthorizer};

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
