use axum::extract::{FromRef, FromRequestParts};
use http::{HeaderValue, header::AUTHORIZATION, request::Parts};
use of_model::contract::{principal_type::PrincipalType, standard_claims::StandardClaims};
use of_model::id::Id;
use of_model::idp::contract::{error_code::ErrorCode, error_response::ErrorResponse};

use crate::router::state::RouterState;

pub const AUTHORIZATION_BEARER_PREFIX: &str = "Bearer ";

pub struct StandardAuthorization {
    pub subject: Id,
    pub introspection_audiences: Vec<String>,
    pub claims: StandardClaims,
    pub application_id: Id,
}

impl StandardAuthorization {
    pub fn require_oidc_audience(&self, service_audience: &str) -> Result<(), ErrorResponse> {
        if self.claims.aud != self.claims.client_id && self.claims.aud != service_audience {
            return Err(ErrorResponse::new(ErrorCode::NotAuthorized));
        }
        Ok(())
    }
}

impl<S> FromRequestParts<S> for StandardAuthorization
where
    RouterState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ErrorResponse;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if let Some(authorization_header_value) = parts.headers.get(AUTHORIZATION) {
            let authorization_string = authorization_from_header(authorization_header_value)?;
            let router_state = RouterState::from_ref(state);
            return authorize_bearer(&router_state, authorization_string).await;
        }
        Err(ErrorResponse::new(ErrorCode::NotAuthorized)
            .with_description("missing authorization header"))
    }
}

pub async fn authorize_bearer(
    router_state: &RouterState,
    authorization_string: &str,
) -> Result<StandardAuthorization, ErrorResponse> {
    authorize_bearer_with_principal(
        router_state,
        authorization_string,
        Some(PrincipalType::User),
    )
    .await
}

pub(crate) async fn authorize_bearer_any_principal(
    router_state: &RouterState,
    authorization_string: &str,
) -> Result<StandardAuthorization, ErrorResponse> {
    authorize_bearer_with_principal(router_state, authorization_string, None).await
}

pub(crate) async fn authorize_bearer_client(
    router_state: &RouterState,
    authorization_string: &str,
) -> Result<StandardAuthorization, ErrorResponse> {
    authorize_bearer_with_principal(
        router_state,
        authorization_string,
        Some(PrincipalType::Client),
    )
    .await
}

async fn authorize_bearer_with_principal(
    router_state: &RouterState,
    authorization_string: &str,
    expected_principal: Option<PrincipalType>,
) -> Result<StandardAuthorization, ErrorResponse> {
    let access = router_state
        .oauth2_service
        .verify_access_token_claims(authorization_string)
        .await
        .map_err(|_| ErrorResponse::new(ErrorCode::NotAuthorized))?;
    let claims = access.standard_claims;
    if expected_principal.is_some_and(|expected| expected != claims.principal_type) {
        return Err(ErrorResponse::new(ErrorCode::NotAuthorized));
    }
    let subject =
        Id::parse_str(&claims.sub).map_err(|_| ErrorResponse::new(ErrorCode::NotAuthorized))?;

    Ok(StandardAuthorization {
        subject,
        claims,
        application_id: access.application_id,
        introspection_audiences: access.introspection_audiences,
    })
}

fn authorization_from_header(
    authorization_header_value: &HeaderValue,
) -> Result<&str, ErrorResponse> {
    log::debug!("parsing authorization header");
    match authorization_header_value.to_str() {
        Ok(authorization_string) => {
            if authorization_string.len() < AUTHORIZATION_BEARER_PREFIX.len() {
                log::warn!(
                    "invalid authorization header is too short: length={}",
                    authorization_string.len()
                );
                return Err(ErrorResponse::new(ErrorCode::NotAuthorized)
                    .with_description("authorization header is too short"));
            }
            if !authorization_string.starts_with(AUTHORIZATION_BEARER_PREFIX) {
                log::warn!(
                    "authorization header does not start with 'Bearer ', starts with: {}",
                    authorization_string.chars().take(10).collect::<String>()
                );
                return Err(ErrorResponse::new(ErrorCode::NotAuthorized)
                    .with_description("authorization header does not start with 'Bearer '"));
            }
            log::debug!("authorization header parsed successfully");
            Ok(&authorization_string[AUTHORIZATION_BEARER_PREFIX.len()..])
        }
        Err(e) => {
            log::warn!(
                "invalid authorization header cannot be parsed as string: {}",
                e
            );
            Err(ErrorResponse::new(ErrorCode::NotAuthorized)
                .with_description("invalid authorization header cannot be parsed as string"))
        }
    }
}
