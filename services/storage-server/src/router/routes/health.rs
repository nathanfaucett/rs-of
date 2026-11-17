use axum::{Json, extract::State};

use crate::router::state::RouterState;

#[utoipa::path(
    get,
    path = "/health",
    responses((status = OK, description = "Server is healthy", body = ()))
)]
pub async fn health(State(_): State<RouterState>) -> Json<()> {
    Json(())
}
