#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use of_model::contract::{
    standard_claims::StandardClaims, token_type::TokenType, token_use::TokenUse,
};
use of_model::id::Id;
use of_model::idp::contract::{
    authorization_code_grant_request::AuthorizationCodeGrantRequest,
    error_code::ErrorCode,
    error_response::{ErrorResponse, ErrorResponseResult},
    jwk_public::JwkPublic,
    jwks::Jwks,
};

use super::jwt::{decode_jwt, verify_jwt};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AccessClaims {
    #[serde(flatten)]
    pub standard_claims: StandardClaims,
    pub application_id: Id,
    pub introspection_audiences: Vec<String>,
}

/// Verify an access JWT against public keys already trusted for this issuer.
pub fn verify_access_jwt(
    jwks: &Jwks,
    issuer: &str,
    audience: &str,
    token: &str,
    now: i64,
) -> ErrorResponseResult<AccessClaims> {
    let (header, _) = decode_jwt::<serde::de::IgnoredAny>(token)?;
    let mut keys = jwks.keys.iter().filter(|key| key.kid == header.kid);
    let key = keys.next().ok_or_else(invalid_access)?;
    if keys.next().is_some() {
        return Err(invalid_access());
    }
    let claims = verify_access_claims(key, issuer, token, now)?;
    if claims.standard_claims.aud != audience {
        return Err(invalid_access());
    }
    Ok(claims)
}

pub(super) fn verify_access_claims(
    key: &JwkPublic,
    issuer: &str,
    token: &str,
    now: i64,
) -> ErrorResponseResult<AccessClaims> {
    let (_, access) = verify_jwt::<AccessClaims>(key, token)?;
    let claims = &access.standard_claims;
    if issuer.is_empty()
        || claims.iss != issuer
        || claims.aud.is_empty()
        || claims.r#type != TokenType::Bearer
        || claims.r#use != TokenUse::Access
        || claims.exp <= now
        || claims.nbf > now
        || claims.iat > now
        || claims.client_id.is_empty()
        || Id::parse_str(&claims.sub).map_or(true, |id| id.is_nil())
        || access.application_id.is_nil()
    {
        return Err(invalid_access());
    }
    Ok(access)
}

fn invalid_access() -> ErrorResponse {
    ErrorResponse::new(ErrorCode::InvalidGrant).with_description("invalid access token claims")
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct PrincipalBoundClaims {
    #[serde(flatten)]
    pub standard_claims: of_model::contract::standard_claims::StandardClaims,
    pub principal_key_id: of_model::id::Id,
}

#[derive(serde::Serialize)]
pub(super) struct RefreshClaims {
    #[serde(flatten)]
    pub standard_claims: of_model::contract::standard_claims::StandardClaims,
    pub principal_key_id: of_model::id::Id,
    pub jti: alloc::string::String,
}

pub fn validate_authorization_code_grant(
    request: &AuthorizationCodeGrantRequest,
    expected_client_id: &str,
    expected_redirect_uri: Option<&str>,
) -> ErrorResponseResult<()> {
    if request.code.is_empty() {
        return Err(
            ErrorResponse::new(ErrorCode::InvalidRequest).with_description("code is required")
        );
    }

    if request.code_verifier.is_empty() {
        return Err(ErrorResponse::new(ErrorCode::InvalidRequest)
            .with_description("code_verifier is required"));
    }

    if let Some(client_id) = &request.client_id
        && client_id != expected_client_id
    {
        return Err(ErrorResponse::new(ErrorCode::InvalidClient));
    }

    if let Some(redirect_uri) = &request.redirect_uri
        && expected_redirect_uri.is_some_and(|expected| expected != redirect_uri)
    {
        return Err(ErrorResponse::new(ErrorCode::InvalidGrant)
            .with_description("redirect_uri does not match the authorization request"));
    }

    Ok(())
}
