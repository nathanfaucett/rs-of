// Generated from the canonical allOf/oneOf composition.
use crate::models;
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct JwkPublic {
    #[serde(flatten)]
    pub parameters: models::JwkPublicParameters,
    #[serde(rename = "alg")]
    pub alg: models::JwsAlgorithm,
    #[serde(rename = "kid")]
    pub kid: String,
    #[serde(rename = "use")]
    pub r#use: models::KeyUse,
}
