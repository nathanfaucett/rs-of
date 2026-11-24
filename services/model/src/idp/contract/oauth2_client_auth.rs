#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::idp::contract::token_endpoint_auth_method::TokenEndpointAuthMethod;

#[derive(Clone, Debug)]
pub struct OAuth2ClientAuth {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub method: TokenEndpointAuthMethod,
}
