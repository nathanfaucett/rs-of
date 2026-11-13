#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec::Vec,
};

use crate::contract::RefreshToken;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct RefreshTokenGrantRequest {
    pub refresh_token: RefreshToken,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}
