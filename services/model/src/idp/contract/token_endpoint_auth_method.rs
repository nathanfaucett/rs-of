use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

use serde::{Deserialize, Serialize};

/// Client token endpoint authentication methods (RFC 7591 / OIDC).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum TokenEndpointAuthMethod {
    /// Client authenticates using the HTTP Basic authentication scheme.
    ClientSecretBasic,
    /// Client authenticates by including the credentials in the request body.
    ClientSecretPost,
    /// Client authenticates using a signed JWT assertion (Asymmetric Crypto).
    PrivateKeyJwt,
    /// Client authenticates using a signed JWT utilizing the shared client secret.
    ClientSecretJwt,
    /// Public client that does not authenticate at the token endpoint (e.g., PKCE only).
    None,
}

impl TokenEndpointAuthMethod {
    /// Whether the implemented token endpoint authenticates with a Client secret verifier.
    #[must_use]
    pub const fn uses_secret_verifier(self) -> bool {
        matches!(self, Self::ClientSecretBasic | Self::ClientSecretPost)
    }
}

impl fmt::Display for TokenEndpointAuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClientSecretBasic => write!(f, "client_secret_basic"),
            Self::ClientSecretPost => write!(f, "client_secret_post"),
            Self::PrivateKeyJwt => write!(f, "private_key_jwt"),
            Self::ClientSecretJwt => write!(f, "client_secret_jwt"),
            Self::None => write!(f, "none"),
        }
    }
}
