use std::{io, path::PathBuf};

use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use idp_model::contract::{JwkPublic, JwkPublicParameters, JwsAlgorithm, KeyUse};
use idp_service::repo::{PrivateKeyKeyringRepo, PrivateKeyRepo};
use iroh::{EndpointId, SecretKey, endpoint::presets::Preset};
use iroh_chain::{EndpointIdStore, Server};

use super::{NodeJoinApproval, NodeJoinRequest, read_optional};
use crate::AppConfig;

#[cfg(test)]
#[path = "node_join_transport_tests.rs"]
mod tests;

struct RetainedJoinAdmission {
    data_dir: PathBuf,
    issuer: String,
    namespace: String,
}

impl device_service::join_admission::RetainedJoinAdmission for RetainedJoinAdmission {
    fn pinned_join_peer(
        &self,
        local_endpoint: EndpointId,
        approved: bool,
    ) -> io::Result<Option<EndpointId>> {
        let Some(request) =
            read_optional::<NodeJoinRequest>(&self.data_dir.join("node-join-request.json"))?
        else {
            return Ok(None);
        };
        request.validate()?;
        if request.installation != self.issuer || request.endpoint_id != local_endpoint.to_string()
        {
            return Ok(None);
        }
        let namespace = format!(
            "{}:node:{}:{}",
            self.namespace, request.member_id, request.public_signer.kid
        );
        let Some(private) = PrivateKeyKeyringRepo::new(&self.issuer)
            .map_err(io::Error::other)?
            .load(&namespace, &key::DerivationPath::default())
            .map_err(io::Error::other)?
        else {
            return Ok(None);
        };
        let (x, y) = private.to_xy().map_err(io::Error::other)?;
        let public = JwkPublic {
            kid: request.public_signer.kid.clone(),
            r#use: KeyUse::Signature,
            alg: JwsAlgorithm::ES256K,
            params: JwkPublicParameters::Ec {
                crv: "secp256k1".into(),
                x: STANDARD_NO_PAD.encode(x),
                y: STANDARD_NO_PAD.encode(y),
            },
        };
        if public != request.public_signer {
            return Ok(None);
        }
        if approved {
            let Some(approval) =
                read_optional::<NodeJoinApproval>(&self.data_dir.join("node-join-approval.json"))?
            else {
                return Ok(None);
            };
            approval.verify(&request)?;
        }
        Ok(Some(
            request.approving_peer.parse().map_err(io::Error::other)?,
        ))
    }
}

/// Adds pinned pairing admission for prepared joins and bootstrap admission for approved joins.
pub async fn bind_node_join_endpoint<P: Preset>(
    config: &AppConfig,
    preset: P,
    secret: SecretKey,
    allowed: EndpointIdStore,
) -> io::Result<Server> {
    device_service::join_admission::bind_node_join_endpoint(
        preset,
        secret,
        allowed,
        RetainedJoinAdmission {
            data_dir: config.data_dir.clone().into(),
            issuer: config.oauth2.issuer.clone(),
            namespace: config.key_namespace.clone(),
        },
    )
    .await
}
