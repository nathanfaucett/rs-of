use of_model::id::Id;
use of_model::idp::contract::error_code::ErrorCode;
use of_model::idp::contract::error_response::ErrorResponse;
use of_model::idp::contract::permission_evaluation::IdentityAction;
use of_model::idp::contract::permission_evaluation::PermissionEvaluationRequest;
use of_model::idp::contract::permission_evaluation::PermissionSubject;
use of_model::idp::contract::permission_evaluation::PermissionTarget;

use crate::router::middleware::device_authorization::DeviceAuthorization;
use crate::router::state::RouterState;

pub(crate) async fn require_identity_permission(
    state: &RouterState,
    actor: &DeviceAuthorization,
    action: IdentityAction,
    target: PermissionTarget,
) -> Result<(), ErrorResponse> {
    if actor.subject.is_nil() || !matches!(target, PermissionTarget::Installation { .. }) {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    state
        .control_plane
        .active_user(actor.subject)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::AccessDenied))?;
    let allowed = state
        .management_service
        .evaluate_permission(&PermissionEvaluationRequest {
            request_id: Id::now_v7(),
            subject: PermissionSubject::User { id: actor.subject },
            action,
            target,
        })
        .await
        .map_err(ErrorResponse::from)?;
    if !allowed {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied));
    }
    Ok(())
}
