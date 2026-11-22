use std::{fs, io, io::Write, path::Path};

use device_service::hosted_join::{NodeJoinApprovalExt, NodeJoinRequestExt};

use idp_service::{oauth2::node_signer::NodeSignerBinding, repo::private_key_repo::PrivateKeyRepo};
use iroh::EndpointId;
use of_model::id::Id;
use serde::Serialize;

use of_model::node_join::NodeJoinApproval;
use of_model::node_join::NodeJoinRequest;

use crate::provisioning::OwnerProvisioner;

#[path = "node_join_bootstrap.rs"]
pub mod bootstrap_state;
#[path = "node_join_transport.rs"]
pub mod transport;

impl<P: PrivateKeyRepo + Send + Sync> OwnerProvisioner<P> {
    /// The peer must be explicitly trusted by the local setup operator, never taken from discovery.
    pub fn prepare_node_join(
        &self,
        binding: &NodeSignerBinding,
        name: &str,
        peer: EndpointId,
    ) -> io::Result<NodeJoinRequest> {
        let path = self.data_dir.join("node-join-request.json");
        let request = NodeJoinRequest {
            purpose: "of.equal-node-join.v1".into(),
            installation: self.oauth2.oauth_config.issuer.clone(),
            member_id: binding.member_id,
            endpoint_id: binding.endpoint_id.clone(),
            name: name.into(),
            approving_peer: peer.to_string(),
        };
        request.validate()?;
        persist_exact(&path, &request)?;
        Ok(request)
    }

    /// Receipt persistence is not local membership, owner-policy bootstrap or completed readiness.
    pub fn record_node_join_approval(&self, approval: &NodeJoinApproval) -> io::Result<()> {
        let request: NodeJoinRequest =
            read_optional(&self.data_dir.join("node-join-request.json"))?
                .ok_or_else(|| invalid("no persisted join request"))?;
        approval.verify(&request)?;
        persist_exact(&self.data_dir.join("node-join-approval.json"), approval)
    }

    pub fn node_join_request(&self) -> io::Result<Option<NodeJoinRequest>> {
        let request: Option<NodeJoinRequest> =
            read_optional(&self.data_dir.join("node-join-request.json"))?;
        if let Some(request) = &request {
            request.validate()?;
        }
        Ok(request)
    }

    pub fn node_join_approval(&self) -> io::Result<Option<NodeJoinApproval>> {
        let approval: Option<NodeJoinApproval> =
            read_optional(&self.data_dir.join("node-join-approval.json"))?;
        if let Some(approval) = &approval {
            let request = read_optional(&self.data_dir.join("node-join-request.json"))?
                .ok_or_else(|| invalid("approval has no persisted request"))?;
            approval.verify(&request)?;
        }
        Ok(approval)
    }
}

pub fn read_optional<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            serde_json::from_slice(&fs::read(path)?)
                .map(Some)
                .map_err(io::Error::other)
        }
        Ok(_) => Err(invalid("join journal must be a regular file")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn persist_exact<T: Serialize + serde::de::DeserializeOwned + PartialEq>(
    path: &Path,
    value: &T,
) -> io::Result<()> {
    if let Some(previous) = read_optional::<T>(path)? {
        return if previous == *value {
            fs::File::open(
                path.parent()
                    .ok_or_else(|| invalid("join journal has no parent"))?,
            )?
            .sync_all()
        } else {
            Err(invalid("persisted join journal conflicts"))
        };
    }
    let temporary = path.with_extension(format!("{}.tmp", Id::now_v7()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&serde_json::to_vec(value)?)?;
        file.sync_all()?;
        match fs::hard_link(&temporary, path) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if read_optional::<T>(path)?.as_ref() != Some(value) {
                    return Err(invalid("concurrent join journal conflicts"));
                }
            }
            Err(error) => return Err(error),
        }
        fs::File::open(
            path.parent()
                .ok_or_else(|| invalid("join journal has no parent"))?,
        )?
        .sync_all()
    })();
    let cleanup = fs::remove_file(&temporary);
    result.and(cleanup)
}

pub fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
