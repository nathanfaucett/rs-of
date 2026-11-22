use axum::{Json, extract::State};
use of_model::idp::contract::{
    error_response::ErrorResponse,
    jwk_public::JwkPublic,
    permission_evaluation::{IdentityAction, IdentityResource, PermissionTarget},
};

use crate::router::{
    middleware::{
        authorization::StandardAuthorization, identity_administration::require_identity_permission,
    },
    state::RouterState,
};

#[utoipa::path(
    post,
    path = "/issuer-keys/rotate",
    responses((status = 200, description = "New public issuer key", body = JwkPublic),
        (status = 403, description = "Rotation not authorized on this IdP")),
    security(("authorization" = [])),
)]
pub async fn rotate_issuer_key(
    State(state): State<RouterState>,
    actor: StandardAuthorization,
) -> Result<Json<JwkPublic>, ErrorResponse> {
    require_identity_permission(
        &state,
        &actor,
        IdentityAction::KeysRotate,
        PermissionTarget::Installation {
            resource: IdentityResource::IdpSecurity,
        },
    )
    .await?;
    state.oauth2_service.rotate_issuer_key().await.map(Json)
}
