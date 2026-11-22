use axum::{Json, extract::State};
use of_model::idp::contract::{
    approve_for_user_request::ApproveForUserRequest, error_response::ErrorResponse,
    is_allowed_for_user_request::IsAllowedForUserRequest,
    is_allowed_for_user_response::IsAllowedForUserResponse,
};

use crate::router::{middleware::authorization::StandardAuthorization, state::RouterState};

#[utoipa::path(
    post,
    path = "/oauth2/auth/allowed-for-user",
    request_body(content = IsAllowedForUserRequest, content_type = "application/json"),
    responses((status = 200, description = "User approval status", body = IsAllowedForUserResponse)),
    security(
        ("authorization" = [])
    )
)]
pub(crate) async fn is_allowed_for_user(
    State(state): State<RouterState>,
    actor: StandardAuthorization,
    Json(request): Json<IsAllowedForUserRequest>,
) -> Result<Json<IsAllowedForUserResponse>, ErrorResponse> {
    actor.require_oidc_audience(&state.service_audience)?;
    let principal = state
        .oauth2_service
        .active_user_principal(actor.subject)
        .await?;
    let response = state
        .oauth2_service
        .is_allowed_for_user(request, principal.as_ref())
        .await?;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/oauth2/auth/approve-for-user",
    request_body(content = ApproveForUserRequest, content_type = "application/json"),
    responses((status = 200, description = "Client approved", body = IsAllowedForUserResponse)),
    security(
        ("authorization" = [])
    )
)]
pub(crate) async fn approve_for_user(
    State(state): State<RouterState>,
    actor: StandardAuthorization,
    Json(request): Json<ApproveForUserRequest>,
) -> Result<Json<IsAllowedForUserResponse>, ErrorResponse> {
    actor.require_oidc_audience(&state.service_audience)?;
    let principal = state
        .oauth2_service
        .active_user_principal(actor.subject)
        .await?;
    let response = state
        .oauth2_service
        .approve_for_user(request, principal.as_ref())
        .await?;
    Ok(Json(response))
}
