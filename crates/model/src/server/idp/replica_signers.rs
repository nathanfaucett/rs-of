use alloc::string::String;

use serde::Deserialize;

use crate::idp::contract::JwkPublic;
use crate::idp::model::Id;

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnrollReplicaSigner {
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub member_id: Id,
    pub endpoint_id: String,
    pub public_jwk: JwkPublic,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RotateReplicaSigner {
    pub public_jwk: JwkPublic,
}
