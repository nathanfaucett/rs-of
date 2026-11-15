use of_model::idp::contract::{
    error_code::ErrorCode, error_response::ErrorResponse,
    introspection_response::IntrospectionResponse,
};

use crate::router::{
    middleware::authorization::authorize_bearer_any_principal, state::RouterState,
};

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

    Ok(IntrospectionResponse {
        claims: inspected.claims,
        application_id: inspected.application_id.to_string(),
    })
}
