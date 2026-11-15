use axum::{Json, extract::State};
use of_model::idp::contract::{
    entity_type::EntityType, error_code::ErrorCode, error_response::ErrorResponse,
    user_info::UserInfo,
};

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
    StandardAuthorization { principal, .. }: StandardAuthorization,
) -> Result<Json<UserInfo>, ErrorResponse> {
    if principal.get_entity_type() != EntityType::User {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied)
            .with_description("Only users can access this endpoint".to_string()));
    }

    let user_info = state
        .oauth2_service
        .find_user_info(principal.get_entity_id())
        .await?;

    Ok(Json(user_info))
}
