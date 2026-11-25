use axum::{
    Json,
    extract::{Path, State},
};
use management_service::service::ROLES_READ_PERMISSION;
use of_model::idp::contract::error_response::ErrorResponse;
use of_model::server::management::users::UserApplicationRoleResponse;

use crate::router::{
    middleware::management_authorization::ManagementAuthorization, state::RouterState,
};

use super::roles::require_application_permission;

const USERS_READ_PERMISSION: &str = "users.read";

#[utoipa::path(
    get,
    path = "/users/{user_id}/roles",
    params(
        ("user_id" = String, Path, description = "User ID")
    ),
    responses((status = 200, description = "List assigned Application roles visible to the caller", body = [UserApplicationRoleResponse])),
    security(
        ("authorization" = [])
    )
)]
pub(crate) async fn list_user_roles_across_applications(
    State(state): State<RouterState>,
    Path(user_id): Path<of_model::id::Id>,
    authorization: ManagementAuthorization,
) -> Result<Json<Vec<UserApplicationRoleResponse>>, ErrorResponse> {
    if authorization.installation_policy_scope
        && state
            .management_service
            .has_installation_policy_permission(authorization.subject, ROLES_READ_PERMISSION)
            .await
            .map_err(ErrorResponse::from)?
    {
        let roles = state
            .management_service
            .list_user_roles_across_applications(user_id)
            .await
            .map_err(ErrorResponse::from)?;
        return Ok(Json(
            roles
                .into_iter()
                .filter(|role| !role.application_id.is_nil())
                .map(Into::into)
                .collect(),
        ));
    }

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
