use std::{io, path::PathBuf, sync::Arc};

use axum::{
    Router,
    extract::{Request, State},
    http::{
        HeaderValue, Method, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE, ORIGIN},
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use crate::{SetupAdmission, SetupScope, setup_admission_middleware, setup_files::require_absent};

struct SetupState {
    admission: Arc<SetupAdmission>,
    absent_roots: Vec<PathBuf>,
    scope: SetupScope,
}

async fn healthy(state: Arc<SetupState>) -> bool {
    matches!(
        tokio::task::spawn_blocking(move || {
            state.admission.health().map_err(io::Error::other)?;
            for root in &state.absent_roots {
                require_absent(root)?;
            }
            Ok::<(), io::Error>(())
        })
        .await,
        Ok(Ok(()))
    )
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub(crate) struct SetupStatus {
    pub(crate) stage: String,
    pub(crate) state: String,
    pub(crate) ready: bool,
    pub(crate) owners: Vec<String>,
}

pub fn setup_origin(value: &str) -> io::Result<HeaderValue> {
    let url =
        reqwest::Url::parse(value).map_err(|_| io::Error::other("setup UI origin is invalid"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.origin().ascii_serialization() != value
    {
        return Err(io::Error::other(
            "setup UI origin must be an exact HTTPS origin without credentials or path",
        ));
    }
    HeaderValue::from_str(value).map_err(|_| io::Error::other("setup UI origin is invalid"))
}

/// Owner-relative routes; the composing host applies its prefix exactly once.
/// Only serve on the verified TLS listener, never an unverified proxy/plaintext router.
pub fn setup_router(
    admission: Arc<SetupAdmission>,
    origin: HeaderValue,
    absent_roots: Vec<PathBuf>,
    scope: SetupScope,
) -> Router {
    let state = Arc::new(SetupState {
        admission: admission.clone(),
        absent_roots,
        scope,
    });
    let cors = CorsLayer::new()
        .allow_origin(origin.clone())
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_private_network(true);
    Router::new()
        .route("/setup/owner/status", get(status))
        .with_state(state.clone())
        .merge(
            Router::new()
                .route("/setup/owner/admission", post(check_admission))
                .route_layer(middleware::from_fn_with_state(
                    admission,
                    setup_admission_middleware,
                )),
        )
        .layer(cors)
        .layer(middleware::from_fn_with_state(origin, setup_origin_guard))
}

#[utoipa::path(get, path = "/setup/owner/status", operation_id = "status", responses((status = 200, body = SetupStatus, description = "Awaiting owner setup"), (status = 503, body = SetupStatus, description = "Owner state unavailable")))]
async fn status(State(state): State<Arc<SetupState>>) -> (StatusCode, axum::Json<SetupStatus>) {
    let scope = state.scope;
    let healthy = healthy(state).await;
    (
        if healthy {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        axum::Json(SetupStatus {
            stage: "installation".into(),
            state: if healthy {
                "awaiting_choice"
            } else {
                "unavailable"
            }
            .into(),
            ready: false,
            owners: scope.owners().iter().map(|owner| (*owner).into()).collect(),
        }),
    )
}

pub async fn setup_origin_guard(
    State(allowed): State<HeaderValue>,
    request: Request,
    next: Next,
) -> Response {
    let mut origins = request.headers().get_all(ORIGIN).iter();
    if let Some(origin) = origins.next()
        && (origin != allowed || origins.next().is_some())
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(request).await
}

#[utoipa::path(post, path = "/setup/owner/admission", operation_id = "admission", security(("setup_code" = [])), responses((status = 204, description = "Owner-local Setup admission accepted"), (status = 401, description = "Setup code required"), (status = 403, description = "Denied")))]
async fn check_admission() -> StatusCode {
    StatusCode::NO_CONTENT
}

#[derive(utoipa::OpenApi)]
#[openapi(paths(status, check_admission))]
struct OwnerSetupDoc;
pub fn owner_setup_openapi() -> utoipa::openapi::OpenApi {
    let mut document = <OwnerSetupDoc as utoipa::OpenApi>::openapi();
    utoipa::Modify::modify(&crate::independent_openapi::SetupSecurity, &mut document);
    document
}
