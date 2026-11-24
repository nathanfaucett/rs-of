use axum::{Json, extract::State, http::StatusCode};
use of_model::server::device::device::Device;

use crate::router::state::RouterState;

#[utoipa::path(
    get,
    path = "/device",
    responses((status = 200, description = "Local endpoint public key and address; not a Device record ID or proof of approval", body = Device))
)]
pub(crate) async fn device(State(state): State<RouterState>) -> Result<Json<Device>, StatusCode> {
    Ok(Json(Device {
        public_key: state
            .control_plane
            .device_identity()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .endpoint_id()
            .to_string(),
        address: state
            .control_plane
            .device_identity()
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .endpoint_address()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    }))
}
