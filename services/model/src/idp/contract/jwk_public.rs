#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::{Deserialize, Serialize};

use crate::idp::contract::jwk_public_parameters::JwkPublicParameters;
use crate::idp::contract::jws_algorithm::JwsAlgorithm;
use crate::idp::contract::key_use::KeyUse;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct JwkPublic {
    /// Key use (sig, enc)
    #[serde(rename = "use")]
    pub r#use: KeyUse,

    /// Key ID
    pub kid: String,

    /// Algorithm intended for use
    pub alg: JwsAlgorithm,

    #[serde(flatten)]
    pub params: JwkPublicParameters,
}
