#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

#[derive(Clone, Debug)]
pub struct OAuth2ClientAuth {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub method: crate::idp::contract::token_endpoint_auth_method::TokenEndpointAuthMethod,
}
