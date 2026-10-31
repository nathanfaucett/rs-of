use axum::{Router, extract::Request, http::StatusCode, response::IntoResponse};
use tower::ServiceExt;

/// Routers retain their owner prefixes; composition never nests them a second time.
pub fn compose_router(idp: Router, management: Router, device: Router, storage: Router) -> Router {
    Router::new().fallback(move |request: Request| {
        let unsupported = request.uri().path().starts_with("/management/setup/");
        let router = match request.uri().path().split('/').nth(1) {
            Some("idp") => Some(idp.clone()),
            Some("management") => Some(management.clone()),
            Some("device") => Some(device.clone()),
            Some("storage") => Some(storage.clone()),
            _ => None,
        };
        async move {
            if unsupported {
                return StatusCode::NOT_FOUND.into_response();
            }
            match router {
                Some(router) => router
                    .oneshot(request)
                    .await
                    .expect("infallible owner router"),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    })
}
