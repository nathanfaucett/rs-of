#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::{Deserialize, Serialize};

use crate::idp::contract::code_challenge_method::CodeChallengeMethod;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct PushedAuthorizationRequest {
    pub client_id: String,
    pub redirect_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: CodeChallengeMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
}
