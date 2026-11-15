use axum::extract::State;
use of_model::idp::contract::{error_code::ErrorCode, error_response::ErrorResponse};

use crate::router::state::RouterState;

#[utoipa::path(get, path = "/oauth2/sessions/logout", responses((status = 302, description = "Logout redirect")))]
pub(crate) async fn sessions_logout(
    State(_state): State<RouterState>,
) -> Result<(), ErrorResponse> {
    Err(ErrorResponse::new(ErrorCode::NotImplemented)
        .with_description("endpoint is not implemented"))
}
