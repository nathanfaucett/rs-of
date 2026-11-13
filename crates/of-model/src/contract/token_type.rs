#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum TokenType {
    /// Case-insensitive matches for known specifications
    #[serde(rename = "Bearer", alias = "bearer")]
    Bearer,
    /// DPoP (Demonstrating Proof-of-Possession) is a mechanism that binds an access token to a specific client and request
    #[serde(rename = "DPoP", alias = "dpop")]
    DPoP,
}
