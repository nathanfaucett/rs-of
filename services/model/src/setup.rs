use serde::{Deserialize, Serialize};

use crate::setup as models;

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct Challenge {
    #[serde(rename = "challenge")]
    pub challenge: String,
    #[serde(rename = "operation")]
    pub operation: Box<models::IndependentCreateIntent>,
    #[serde(rename = "recipient")]
    pub recipient: String,
    #[serde(rename = "registration", deserialize_with = "Option::deserialize")]
    pub registration: Option<serde_json::Value>,
}

impl Challenge {
    #[must_use]
    pub fn new(
        challenge: String,
        operation: models::IndependentCreateIntent,
        recipient: String,
        registration: Option<serde_json::Value>,
    ) -> Self {
        Self {
            challenge,
            operation: Box::new(operation),
            recipient,
            registration,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct IndependentCreateIntent {
    #[serde(rename = "administrator")]
    pub administrator: String,
    #[serde(rename = "device")]
    pub device: String,
    #[serde(rename = "idp")]
    pub idp: String,
    #[serde(rename = "issuer")]
    pub issuer: String,
    #[serde(rename = "node")]
    pub node: String,
    #[serde(rename = "operation_nonce")]
    pub operation_nonce: String,
    #[serde(
        rename = "peer",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub peer: Option<Option<String>>,
    #[serde(rename = "purpose")]
    pub purpose: String,
    #[serde(rename = "storage")]
    pub storage: String,
}

impl IndependentCreateIntent {
    #[must_use]
    pub const fn new(
        administrator: String,
        device: String,
        idp: String,
        issuer: String,
        node: String,
        operation_nonce: String,
        purpose: String,
        storage: String,
    ) -> Self {
        Self {
            administrator,
            device,
            idp,
            issuer,
            node,
            operation_nonce,
            peer: None,
            purpose,
            storage,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct IndependentCreateRequest {
    #[serde(rename = "administrator")]
    pub administrator: String,
    #[serde(rename = "device")]
    pub device: String,
    #[serde(rename = "idp")]
    pub idp: String,
    #[serde(rename = "issuer")]
    pub issuer: String,
    #[serde(rename = "node")]
    pub node: String,
    #[serde(rename = "operation_nonce")]
    pub operation_nonce: String,
    #[serde(rename = "peer", skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    #[serde(rename = "purpose")]
    pub purpose: String,
    #[serde(rename = "storage")]
    pub storage: String,
    #[serde(
        rename = "password",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub password: Option<Option<String>>,
}

impl IndependentCreateRequest {
    #[must_use]
    pub const fn new(
        administrator: String,
        device: String,
        idp: String,
        issuer: String,
        node: String,
        operation_nonce: String,
        purpose: String,
        storage: String,
    ) -> Self {
        Self {
            administrator,
            device,
            idp,
            issuer,
            node,
            operation_nonce,
            peer: None,
            purpose,
            storage,
            password: None,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct IndependentOwnerPublic {
    #[serde(rename = "clients")]
    pub clients: Vec<serde_json::Value>,
    #[serde(rename = "facts", deserialize_with = "Option::deserialize")]
    pub facts: Option<serde_json::Value>,
    #[serde(rename = "operation")]
    pub operation: Box<models::IndependentCreateIntent>,
    #[serde(rename = "owner")]
    pub owner: String,
    #[serde(rename = "registered")]
    pub registered: Vec<String>,
}

impl IndependentOwnerPublic {
    #[must_use]
    pub fn new(
        clients: Vec<serde_json::Value>,
        facts: Option<serde_json::Value>,
        operation: models::IndependentCreateIntent,
        owner: String,
        registered: Vec<String>,
    ) -> Self {
        Self {
            clients,
            facts,
            operation: Box::new(operation),
            owner,
            registered,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct IndependentPeer {
    #[serde(rename = "address", deserialize_with = "Option::deserialize")]
    pub address: Option<serde_json::Value>,
    #[serde(rename = "device")]
    pub device: String,
    #[serde(rename = "endpoint")]
    pub endpoint: String,
    #[serde(rename = "idp")]
    pub idp: String,
    #[serde(rename = "issuer")]
    pub issuer: String,
    #[serde(rename = "nonce")]
    pub nonce: String,
    #[serde(rename = "signature")]
    pub signature: String,
}

impl IndependentPeer {
    #[must_use]
    pub const fn new(
        address: Option<serde_json::Value>,
        device: String,
        endpoint: String,
        idp: String,
        issuer: String,
        nonce: String,
        signature: String,
    ) -> Self {
        Self {
            address,
            device,
            endpoint,
            idp,
            issuer,
            nonce,
            signature,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependentJoinRequest {
    #[serde(rename = "device_id")]
    pub device_id: uuid::Uuid,
    #[serde(rename = "endpoint_id")]
    pub endpoint_id: String,
    #[serde(rename = "member_id")]
    pub member_id: uuid::Uuid,
    #[serde(rename = "operation")]
    pub operation: Box<models::IndependentCreateIntent>,
    #[serde(rename = "source")]
    pub source: Box<models::IndependentPeer>,
}

impl IndependentJoinRequest {
    #[must_use]
    pub fn new(
        device_id: uuid::Uuid,
        endpoint_id: String,
        member_id: uuid::Uuid,
        operation: models::IndependentCreateIntent,
        source: models::IndependentPeer,
    ) -> Self {
        Self {
            device_id,
            endpoint_id,
            member_id,
            operation: Box::new(operation),
            source: Box::new(source),
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndependentJoinApproval {
    #[serde(rename = "owner_subject")]
    pub owner_subject: uuid::Uuid,
    #[serde(rename = "request")]
    pub request: Box<models::IndependentJoinRequest>,
    #[serde(rename = "signature")]
    pub signature: String,
}

impl IndependentJoinApproval {
    #[must_use]
    pub fn new(
        owner_subject: uuid::Uuid,
        request: models::IndependentJoinRequest,
        signature: String,
    ) -> Self {
        Self {
            owner_subject,
            request: Box::new(request),
            signature,
        }
    }
}
