use crate::router::{ManagementRouterService, middleware::DeviceAuthorization};
use idp_model::contract::{ErrorCode, ErrorResponse};

pub(crate) async fn require_application_permission(
    management_service: &ManagementRouterService,
    authorization: &DeviceAuthorization,
    application_id: idp_model::model::Id,
    permission: &str,
) -> Result<(), ErrorResponse> {
    if application_id.is_nil() || authorization.application_id != application_id {
        return Err(ErrorResponse::new(ErrorCode::AccessDenied)
            .with_description("application permission does not authorize another application"));
    }
    let has_permission = management_service
        .has_user_application_permission(authorization.subject, application_id, permission)
        .await
        .map_err(ErrorResponse::from)?;

    if has_permission {
        return Ok(());
    }

    Err(ErrorResponse::new(ErrorCode::AccessDenied)
        .with_description("missing required application permission"))
}
