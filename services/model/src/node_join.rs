use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::idp::contract::jwk_public::JwkPublic;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct NodeJoinRequest {
    pub purpose: String,
    pub installation: String,
    #[cfg_attr(feature = "utoipa", schema(value_type = String, format = "uuid"))]
    pub member_id: Id,
    pub endpoint_id: String,
    pub name: String,
    pub public_signer: JwkPublic,
    pub approving_peer: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct NodeJoinApproval {
    pub request: NodeJoinRequest,
    #[cfg_attr(feature = "utoipa", schema(value_type = String, format = "uuid"))]
    pub device_id: Id,
    #[cfg_attr(feature = "utoipa", schema(value_type = String, format = "uuid"))]
    pub owner_subject: Id,
    pub signature: String,
}
