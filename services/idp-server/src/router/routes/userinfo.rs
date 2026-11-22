use axum::{Json, extract::State};
use of_model::idp::contract::{error_response::ErrorResponse, user_info::UserInfo};

use crate::router::{middleware::authorization::StandardAuthorization, state::RouterState};

#[utoipa::path(
    get,
    path = "/userinfo",
    responses((status = 200, description = "Userinfo", body = UserInfo)),
    security(
        ("authorization" = [])
    )
)]
pub(crate) async fn userinfo(
    State(state): State<RouterState>,
    actor: StandardAuthorization,
) -> Result<Json<UserInfo>, ErrorResponse> {
    actor.require_oidc_audience(&state.service_audience)?;

    let user_info = state.oauth2_service.find_user_info(actor.subject).await?;

    Ok(Json(user_info))
}
