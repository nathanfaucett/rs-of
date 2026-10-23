use std::io;

use api::{IndependentCreateIntent, IndependentPeer};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use idp_model::{contract::JwkPublic, model::Id};
use iroh::{EndpointId, Signature};
use serde::{Deserialize, Serialize};

use crate::DeviceIdentity;

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct IndependentJoinRequest {
    pub operation: IndependentCreateIntent,
    #[cfg_attr(feature = "utoipa", schema(value_type = String, format = "uuid"))]
    pub device_id: Id,
    #[cfg_attr(feature = "utoipa", schema(value_type = String, format = "uuid"))]
    pub member_id: Id,
    pub endpoint_id: String,
    pub public_signer: JwkPublic,
    pub source: IndependentPeer,
}

impl IndependentJoinRequest {
    pub fn validate(&self) -> io::Result<()> {
        self.operation.canonical()?;
        idp_service::replica::DbReplicaSignerRepo::validate_public_key(&self.public_signer)
            .map_err(io::Error::other)?;
        if self.operation.purpose != "join" {
            return Err(io::Error::other("Join purpose required"));
        }
        self.source.verify(
            self.operation
                .peer
                .as_deref()
                .ok_or_else(|| io::Error::other("trusted peer missing"))?,
            &self.source.nonce,
        )?;
        let endpoint: EndpointId = self.endpoint_id.parse().map_err(io::Error::other)?;
        let key: Id = self.public_signer.kid.parse().map_err(io::Error::other)?;
        if self.operation.issuer != self.source.issuer
            || self.endpoint_id == self.source.endpoint
            || endpoint.to_string() != self.endpoint_id
            || self.device_id.get_version_num() != 7
            || self.member_id.get_version_num() != 7
            || key.get_version_num() != 7
            || self.device_id == self.member_id
            || self.device_id == key
            || self.member_id == key
        {
            return Err(io::Error::other("invalid independent Join binding"));
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct IndependentJoinApproval {
    pub request: IndependentJoinRequest,
    #[cfg_attr(feature = "utoipa", schema(value_type = String, format = "uuid"))]
    pub owner_subject: Id,
    pub signature: String,
}

impl IndependentJoinApproval {
    fn payload(&self) -> io::Result<Vec<u8>> {
        Ok(serde_json::to_vec(&(
            "of.independent-join-approval.v1",
            &self.request,
            self.owner_subject,
        ))?)
    }

    pub fn sign(
        identity: &DeviceIdentity,
        request: IndependentJoinRequest,
        owner_subject: Id,
    ) -> io::Result<Self> {
        request.validate()?;
        if identity.endpoint_id().to_string() != request.source.endpoint || owner_subject.is_nil() {
            return Err(io::Error::other("approval identity mismatch"));
        }
        let mut approval = Self {
            request,
            owner_subject,
            signature: String::new(),
        };
        approval.signature = URL_SAFE_NO_PAD.encode(identity.sign(&approval.payload()?).to_bytes());
        Ok(approval)
    }

    pub async fn verify_current(&self, context: &str) -> io::Result<()> {
        self.verify()?;
        if !matches!(context, "device" | "idp") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid shared Join context",
            ));
        }
        api::LocalTls::verification_client()?
            .post(format!(
                "{}/independent/node-joins/verify",
                self.request.source.device
            ))
            .json(&serde_json::json!({"approval": self, "context": context}))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?;
        Ok(())
    }

    pub fn verify(&self) -> io::Result<()> {
        self.request.validate()?;
        if self.owner_subject.is_nil() {
            return Err(io::Error::other("approval owner missing"));
        }
        let peer: EndpointId = self
            .request
            .source
            .endpoint
            .parse()
            .map_err(io::Error::other)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(&self.signature)
            .map_err(io::Error::other)?;
        let signature = Signature::try_from(bytes.as_slice()).map_err(io::Error::other)?;
        peer.verify(&self.payload()?, &signature)
            .map_err(io::Error::other)
    }
}
