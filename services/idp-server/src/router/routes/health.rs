use axum::{Json, extract::State};
use of_model::contract::{health_response::HealthResponse, health_status::HealthStatus};

use crate::router::state::RouterState;

#[utoipa::path(
    get,
    path = "/health",
    responses((status = OK, description = "Server is healthy", body = HealthResponse))
)]
pub(crate) async fn health(State(_state): State<RouterState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        database: HealthStatus::Healthy,
    })
}
