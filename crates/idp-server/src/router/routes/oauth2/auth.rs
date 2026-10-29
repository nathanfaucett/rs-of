use axum::{
    Json,
    extract::{Query, RawQuery, State},
    response::Redirect,
};
use idp_model::contract::{
    ApplicationRegistration, AuthorizationCodeResponse, AuthorizationRequest,
    AuthorizationUiRequest, ErrorCode, ErrorResponse, ResponseType,
};
use serde::Deserialize;

use crate::router::{RouterState, middleware::StandardAuthorization};

#[utoipa::path(
    post,
    path = "/oauth2/auth",
    request_body(content = AuthorizationRequest, content_type = "application/json"),
    responses((status = 200, description = "Authorization response", body = AuthorizationCodeResponse)),
    security(
        ("authorization" = [])
    )
)]
pub(crate) async fn authorize_json(
    State(state): State<RouterState>,
    StandardAuthorization { principal, .. }: StandardAuthorization,
    Json(request): Json<AuthorizationRequest>,
) -> Result<Json<AuthorizationCodeResponse>, ErrorResponse> {
    let response = state
        .oauth2_service
        .authorize(request, principal.as_ref())
        .await?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/oauth2/auth",
    params(
        AuthorizationUiRequest
    ),
    responses((status = 303, description = "Authorization UI redirect"))
)]
pub(crate) async fn authorize_query(
    State(state): State<RouterState>,
    RawQuery(raw_query): RawQuery,
    Query(query): Query<AuthorizationUiRequest>,
) -> Result<Redirect, ErrorResponse> {
    validate_ui_request(&query)?;
    let mut redirect_url = state.ui_base_uri.clone();
    let query_string = raw_query.ok_or_else(|| ErrorResponse::new(ErrorCode::InvalidRequest))?;

    if !redirect_url.ends_with('/') {
        redirect_url.push('/');
    }
    redirect_url.push_str("authorize");
    redirect_url.push('?');
    redirect_url.push_str(&query_string);

    Ok(Redirect::to(&redirect_url))
}

#[derive(Deserialize)]
struct OnboardingRegistration {
    application: ApplicationRegistration,
    client_id: Option<String>,
}

fn validate_ui_request(request: &AuthorizationUiRequest) -> Result<(), ErrorResponse> {
    let invalid = || ErrorResponse::new(ErrorCode::InvalidRequest);
    if let Some(client_id) = &request.client_id {
        return if client_id.trim().is_empty() {
            Err(invalid())
        } else {
            Ok(())
        };
    }
    let application_uri = request
        .application_id
        .as_deref()
        .filter(|uri| !uri.trim().is_empty())
        .ok_or_else(invalid)?;
    let registration: OnboardingRegistration =
        serde_json::from_str(request.registration.as_deref().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
    if request.response_type != ResponseType::Code
        || registration.application.uri != application_uri
        || registration.client_id.is_some()
    {
        return Err(invalid());
    }
    let redirect_uri = reqwest::Url::parse(request.redirect_uri.as_deref().ok_or_else(invalid)?)
        .map_err(|_| invalid())?;
    if redirect_uri.fragment().is_some()
        || matches!(redirect_uri.scheme(), "javascript" | "data" | "file")
        || (matches!(redirect_uri.scheme(), "http" | "https") && redirect_uri.host_str().is_none())
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
