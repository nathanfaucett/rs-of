use idp_model::contract::{ErrorCode, ErrorResponse, IntrospectionResponse};

use crate::{RouterState, authorize_bearer_any_principal};

pub(crate) async fn authorize_storage_bearer(
    state: &RouterState,
    bearer: &str,
) -> Result<IntrospectionResponse, ErrorResponse> {
    let inspected = authorize_bearer_any_principal(state, bearer).await?;
    if !matches!(
        inspected.claims.aud.as_str(),
        "storage:databases" | "storage:filesystems"
    ) {
        return Err(ErrorResponse::new(ErrorCode::NotAuthorized));
    }
    let application_id = state
        .oauth2_service
        .application_id_for_client(&inspected.claims.client_id)
        .await?;
    Ok(IntrospectionResponse {
        claims: inspected.claims,
        application_id: application_id.to_string(),
    })
}
