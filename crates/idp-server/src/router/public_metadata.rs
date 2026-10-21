use axum::{Router, routing::get};

use super::{openapi::openapi_json, openapi_router::openapi_document, routes::version::version};

/// Serves version and the full API contract without opening runtime state.
/// `api_base_uri` is the complete public owner base; `prefix` only mounts routes.
pub fn public_metadata_router(api_base_uri: &str, prefix: &str) -> Router {
    let routes = Router::new()
        .route("/version", get(version))
        .route("/openapi.json", get(openapi_json))
        .with_state(openapi_document(api_base_uri, ""));

    if prefix.is_empty() || prefix == "/" {
        routes
    } else {
        Router::new().nest(prefix, routes)
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::Value;
    use tower::ServiceExt;

    #[tokio::test]
    async fn public_metadata_is_state_free_and_only_exposes_version_and_openapi() {
        for prefix in ["", "/", "/idp", "/custom/idp"] {
            let base = "https://node.example/custom/idp";
            let router = crate::public_metadata_router(base, prefix);
            let mount = if prefix == "/" { "" } else { prefix };
            for path in ["/version", "/openapi.json"] {
                let response = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(format!("{mount}{path}"))
                            .body(Body::empty())
                            .expect("metadata request"),
                    )
                    .await
                    .expect("metadata response");
                assert_eq!(response.status(), StatusCode::OK);
                let body = to_bytes(response.into_body(), usize::MAX)
                    .await
                    .expect("metadata body");
                let value: Value = serde_json::from_slice(&body).expect("metadata JSON");
                if path == "/version" {
                    assert_eq!(value["name"], env!("CARGO_PKG_NAME"));
                    assert!(value["version"].is_string());
                    assert!(value["build"].is_string());
                } else {
                    assert_eq!(
                        value,
                        serde_json::to_value(crate::openapi_document(base, ""))
                            .expect("full contract")
                    );
                    assert_eq!(value["servers"][0]["url"], base);
                    assert!(value["paths"]["/version"].is_object());
                    assert!(value["paths"]["/openapi.json"].is_object());
                }
            }
            for path in [
                "/health",
                "/.well-known/openid-configuration",
                "/.well-known/jwks.json",
                "/oauth2/token",
                "/missing",
                "/idp/version",
            ] {
                let response = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(format!("{mount}{path}"))
                            .body(Body::empty())
                            .expect("unregistered request"),
                    )
                    .await
                    .expect("unregistered response");
                assert_eq!(response.status(), StatusCode::NOT_FOUND, "{mount}{path}");
            }
        }
    }
}
