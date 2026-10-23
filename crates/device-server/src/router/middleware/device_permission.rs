use idp_model::{
    contract::{
        ErrorCode, ErrorResponse, IdentityAction, PermissionEvaluationRequest, PermissionSubject,
        PermissionTarget,
    },
    model::Id,
};

use super::DeviceAuthorization;
use crate::RouterState;

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
