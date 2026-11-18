
use serde::Deserialize;

use crate::id::Id;
use crate::idp::contract::jwk_public::JwkPublic;

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
