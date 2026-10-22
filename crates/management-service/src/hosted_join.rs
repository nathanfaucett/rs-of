use std::{future::Future, io, pin::Pin};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use idp_model::{contract::JwkPublic, model::Id};
use iroh::{EndpointAddr, EndpointId, Signature};
use iroh_chain::{PairingOffer, Server};
use serde::{Deserialize, Serialize};

const PURPOSE: &str = "of.equal-node-join.v1";

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
    pub owner_subject: Id,
    pub signature: String,
}

impl NodeJoinRequest {
    pub fn validate_trusted_peer(peer: &str) -> io::Result<()> {
        let endpoint: EndpointId = peer.parse().map_err(io::Error::other)?;
        if endpoint.to_string() != peer {
            return Err(invalid("trusted peer endpoint ID must be canonical"));
        }
        Ok(())
    }

    pub fn validate(&self) -> io::Result<()> {
        let endpoint: EndpointId = self.endpoint_id.parse().map_err(io::Error::other)?;
        let peer: EndpointId = self.approving_peer.parse().map_err(io::Error::other)?;
        let key_id: Id = self.public_signer.kid.parse().map_err(io::Error::other)?;
        if self.purpose != PURPOSE
            || self.installation.trim().is_empty()
            || self.member_id.get_version_num() != 7
            || key_id.get_version_num() != 7
            || self.name.trim().is_empty()
            || self.name.len() > 256
            || endpoint.to_string() != self.endpoint_id
            || peer.to_string() != self.approving_peer
            || endpoint == peer
        {
            return Err(invalid("invalid equal-Node join binding"));
        }

        Ok(())
    }

    pub async fn send(&self, server: &Server, peer: EndpointAddr) -> io::Result<NodeJoinApproval> {
        self.validate()?;
        if server.endpoint().id().to_string() != self.endpoint_id
            || peer.id.to_string() != self.approving_peer
        {
            return Err(invalid(
                "join endpoint or pinned peer does not match request",
            ));
        }
        let reply = server
            .send_pairing_offer(peer, &serde_json::to_vec(self)?)
            .await?;
        let approval: NodeJoinApproval = serde_json::from_slice(&reply)?;
        approval.verify(self)?;
        Ok(approval)
    }
}

impl NodeJoinApproval {
    pub fn signed_by(
        identity: &crate::DeviceIdentity,
        request: NodeJoinRequest,
        owner_subject: Id,
    ) -> io::Result<Self> {
        let payload = Self::payload(&request, owner_subject)?;
        Ok(Self {
            request,
            owner_subject,
            signature: URL_SAFE_NO_PAD.encode(identity.sign(&payload).to_bytes()),
        })
    }

    pub fn payload(request: &NodeJoinRequest, owner: Id) -> io::Result<Vec<u8>> {
        Ok(serde_json::to_vec(&(
            "of.equal-node-join-approval.v1",
            request,
            owner,
        ))?)
    }

    pub fn verify(&self, expected: &NodeJoinRequest) -> io::Result<()> {
        expected.validate()?;
        if self.request != *expected || self.owner_subject.is_nil() {
            return Err(invalid(
                "approval does not bind the exact persisted join request and owner",
            ));
        }
        let peer: EndpointId = expected.approving_peer.parse().map_err(io::Error::other)?;
        let signature = URL_SAFE_NO_PAD
            .decode(&self.signature)
            .map_err(io::Error::other)?;
        let signature = Signature::try_from(signature.as_slice()).map_err(io::Error::other)?;
        peer.verify(&Self::payload(expected, self.owner_subject)?, &signature)
            .map_err(io::Error::other)
    }
}

pub type JoinFuture<'a, T> = Pin<Box<dyn Future<Output = io::Result<T>> + Send + 'a>>;

pub trait NodeJoinIdentity: Send + Sync {
    fn accepting<'a>(&'a self, bearer: &'a str, accepting: Option<bool>) -> JoinFuture<'a, bool>;
    fn authorize<'a>(&'a self, bearer: &'a str) -> JoinFuture<'a, ()>;
    fn enroll<'a>(
        &'a self,
        offer: &'a PairingOffer,
        bearer: &'a str,
    ) -> JoinFuture<'a, NodeJoinApproval>;
    fn verify<'a>(&'a self, approval: &'a NodeJoinApproval) -> JoinFuture<'a, ()>;
    fn reply<'a>(
        &'a self,
        offer: &'a PairingOffer,
        bearer: &'a str,
        approval: &'a NodeJoinApproval,
    ) -> JoinFuture<'a, ()>;
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod setup_peer_tests {
    use super::NodeJoinRequest;

    #[test]
    fn trusted_peer_input_requires_a_canonical_endpoint_identity() {
        let peer = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        NodeJoinRequest::validate_trusted_peer(peer).expect("known Ed25519 endpoint identity");
        for invalid in [
            "",
            "discovered-peer",
            "https://peer.test",
            &peer.to_uppercase(),
        ] {
            assert!(NodeJoinRequest::validate_trusted_peer(invalid).is_err());
        }
    }
}
