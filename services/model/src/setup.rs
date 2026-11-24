#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, vec::Vec};

use serde::{Deserialize, Serialize};

use crate::setup as models;

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_field_names,
    reason = "preserve the public challenge wire field"
)]
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
    #[allow(
        clippy::option_option,
        reason = "preserve absent, null, and string password wire states"
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
