// Generated from the canonical allOf/oneOf composition.
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JwkPublic {
    #[serde(flatten)]
    pub parameters: crate::http::idp::jwk_public_parameters::JwkPublicParameters,
    #[serde(rename = "alg")]
    pub alg: crate::http::idp::jws_algorithm::JwsAlgorithm,
    #[serde(rename = "kid")]
    pub kid: String,
    #[serde(rename = "use")]
    pub r#use: crate::http::idp::key_use::KeyUse,
}
