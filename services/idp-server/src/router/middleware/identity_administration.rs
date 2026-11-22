use of_model::contract::principal_type::PrincipalType;
use of_model::id::Id;
use of_model::idp::contract::client_registration::ClientRegistration;
use of_model::idp::contract::error_code::ErrorCode;
use of_model::idp::contract::error_response::ErrorResponse;
use of_model::idp::contract::grant_type::GrantType;
use of_model::idp::contract::permission_evaluation::IdentityAction;
use of_model::idp::contract::permission_evaluation::PermissionEvaluationRequest;
use of_model::idp::contract::permission_evaluation::PermissionSubject;
use of_model::idp::contract::permission_evaluation::PermissionTarget;

use crate::router::{middleware::authorization::StandardAuthorization, state::RouterState};

pub(crate) async fn require_identity_permission(
    state: &RouterState,
    actor: &StandardAuthorization,
    action: IdentityAction,
    target: PermissionTarget,
) -> Result<(), ErrorResponse> {
    require_identity_permission_for_audience(state, actor, &state.service_audience, action, target)
        .await
}

pub(crate) async fn require_identity_permission_for_audience(
    state: &RouterState,
    actor: &StandardAuthorization,
    audience: &str,
    action: IdentityAction,
    target: PermissionTarget,
) -> Result<(), ErrorResponse> {
    if actor.claims.principal_type != PrincipalType::User || actor.claims.aud != audience {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    if let PermissionTarget::Application { application_id, .. } = target
        && (application_id.is_nil() || actor.application_id != application_id)
    {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    let request = PermissionEvaluationRequest {
        request_id: Id::now_v7(),
        subject: PermissionSubject::User { id: actor.subject },
        action,
        target,
    };
    let allowed = if let Some(local) = state.local_permissions.as_ref() {
        local
            .evaluate(&request)
            .await
            .map_err(|_| ErrorResponse::new(ErrorCode::TemporarilyUnavailable))?
    } else if let Some(remote) = state.management_permissions.as_ref() {
        remote.evaluate(state, &request).await?
    } else {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied)
            .with_description("Management permission evaluation is not configured"));
    };
    if !allowed {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    Ok(())
}

pub(crate) fn infrastructure_client(client: &ClientRegistration) -> bool {
    client.allowed_grant_types.is_empty()
        || client
            .allowed_grant_types
            .contains(&GrantType::ClientCredentials)
        || client
            .allowed_scopes
            .iter()
            .any(|scope| scope.starts_with("idp.") || scope.starts_with("management."))
}
