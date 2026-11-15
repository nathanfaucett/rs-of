use alloc::string::String;
// Generated from the canonical allOf/oneOf composition.
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct JwkPublic {
    #[serde(flatten)]
    pub parameters: crate::http::storage::jwk_public_parameters::JwkPublicParameters,
    #[serde(rename = "alg")]
    pub alg: crate::http::storage::jws_algorithm::JwsAlgorithm,
    #[serde(rename = "kid")]
    pub kid: String,
    #[serde(rename = "use")]
    pub r#use: crate::http::storage::key_use::KeyUse,
}
