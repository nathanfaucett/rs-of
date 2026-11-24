#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct IsAllowedForUserRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
}
