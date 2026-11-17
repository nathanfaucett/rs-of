#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

use serde::{Deserialize, Serialize};

use crate::idp::contract::password_grant_request::PasswordGrantRequest;

use crate::idp::contract::authorization_code_grant_request::AuthorizationCodeGrantRequest;
use crate::idp::contract::client_credentials_grant_request::ClientCredentialsGrantRequest;
use crate::idp::contract::refresh_token_grant_request::RefreshTokenGrantRequest;
use crate::idp::contract::token_exchange_grant_request::TokenExchangeGrantRequest;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(tag = "grant_type")]
pub enum TokenRequest {
    #[serde(rename = "password")]
    Password(PasswordGrantRequest),
    #[serde(rename = "authorization_code")]
    AuthorizationCode(AuthorizationCodeGrantRequest),
    #[serde(rename = "client_credentials")]
    ClientCredentials(ClientCredentialsGrantRequest),
    #[serde(rename = "refresh_token")]
    RefreshToken(RefreshTokenGrantRequest),
    #[serde(rename = "urn:ietf:params:oauth:grant-type:token-exchange")]
    TokenExchange(TokenExchangeGrantRequest),
}
