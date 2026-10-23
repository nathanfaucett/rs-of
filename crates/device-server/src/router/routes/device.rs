use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;
use utoipa::ToSchema;

use crate::RouterState;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Device {
    public_key: String,
    address: String,
}

#[utoipa::path(
    get,
    path = "/device",
    responses((status = 200, description = "Local device identity", body = Device))
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
