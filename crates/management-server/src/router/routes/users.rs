use axum::{
    Json,
    extract::{Path, State},
};
use of_model::idp::contract::ErrorResponse;
pub use of_model::server::management::users::UserApplicationRoleResponse;

use crate::router::{RouterState, middleware::ManagementAuthorization};

use super::roles::require_application_permission;

const USERS_READ_PERMISSION: &str = "users.read";

#[utoipa::path(
    get,
    path = "/users/{user_id}/roles",
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    responses((status = 200, description = "List user roles within the authorized application", body = [UserApplicationRoleResponse])),
    security(
        ("authorization" = [])
    )
)]
pub(crate) async fn list_user_roles_across_applications(
    State(state): State<RouterState>,
    Path(user_id): Path<of_model::idp::model::Id>,
    authorization: ManagementAuthorization,
) -> Result<Json<Vec<UserApplicationRoleResponse>>, ErrorResponse> {
    require_application_permission(
        state.management_service.as_ref(),
        &authorization,
        authorization.application_id,
        USERS_READ_PERMISSION,
    )
    .await?;

    let roles = state
        .management_service
        .list_user_roles(authorization.application_id, user_id)
        .await
        .map_err(ErrorResponse::from)?;

    Ok(Json(roles.into_iter().map(Into::into).collect()))
}
