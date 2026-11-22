use std::{io, sync::Arc};

use ofdb_sql::Database;

use idp_service::{oauth2::node_signer::NodeSignerBinding, repo::private_key_repo::PrivateKeyRepo};

use super::{NodeJoinApproval, invalid, persist_exact, read_optional};
use crate::provisioning::OwnerProvisioner;

use device_service::node_join_owner::NodeJoinStage;

fn stage_file(stage: NodeJoinStage) -> &'static str {
    match stage {
        NodeJoinStage::Idp => "node-join-idp-import.json",
        NodeJoinStage::Management => "node-join-policy-import.json",
        NodeJoinStage::PublicReady => "node-join-public-ready.json",
    }
}

impl<P: PrivateKeyRepo + Send + Sync> OwnerProvisioner<P> {
    pub fn node_join_database(&self) -> io::Result<Arc<Database>> {
        self.database.clone().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "remote identity consumers do not own sync history",
            )
        })
    }

    pub fn node_join_anchor(&self) -> io::Result<NodeJoinApproval> {
        let approval = self
            .node_join_approval()?
            .ok_or_else(|| invalid("no verified persisted join approval"))?;
        if approval.request.installation != self.oauth2.oauth_config.issuer {
            return Err(invalid("join approval issuer does not match configuration"));
        }
        Ok(approval)
    }

    /// Owner-local import journal only; this is not permission or a readiness decision.
    pub fn record_node_join_stage(&self, stage: NodeJoinStage) -> io::Result<()> {
        let anchor = self.node_join_anchor()?;
        persist_exact(&self.data_dir.join(stage_file(stage)), &anchor)
    }

    pub fn node_join_stage_recorded(&self, stage: NodeJoinStage) -> io::Result<bool> {
        let anchor = self.node_join_anchor()?;
        let previous: Option<NodeJoinApproval> =
            read_optional(&self.data_dir.join(stage_file(stage)))?;
        match previous {
            Some(previous) if previous != anchor => Err(invalid(
                "join import journal conflicts with persisted anchor",
            )),
            previous => Ok(previous.is_some()),
        }
    }

    /// Missing public rows may be pending; present contradictions and learned revocation deny.
    pub async fn check_node_join_local_binding(&self) -> io::Result<()> {
        self.node_join_anchor().map(|_| ())
    }
}

impl<P: PrivateKeyRepo + Send + Sync> device_service::node_join_owner::NodeJoinOwner
    for OwnerProvisioner<P>
{
    fn node_join_database(&self) -> io::Result<Arc<Database>> {
        OwnerProvisioner::node_join_database(self)
    }
    fn prepare_node_join(
        &self,
        binding: &NodeSignerBinding,
        name: &str,
        peer: iroh::EndpointId,
    ) -> io::Result<of_model::node_join::NodeJoinRequest> {
        OwnerProvisioner::prepare_node_join(self, binding, name, peer)
    }
    fn record_node_join_approval(&self, approval: &NodeJoinApproval) -> io::Result<()> {
        OwnerProvisioner::record_node_join_approval(self, approval)
    }
    fn node_join_request(&self) -> io::Result<Option<of_model::node_join::NodeJoinRequest>> {
        OwnerProvisioner::node_join_request(self)
    }
    fn node_join_approval(&self) -> io::Result<Option<of_model::node_join::NodeJoinApproval>> {
        OwnerProvisioner::node_join_approval(self)
    }
    fn node_join_anchor(&self) -> io::Result<NodeJoinApproval> {
        OwnerProvisioner::node_join_anchor(self)
    }
    fn record_node_join_stage(&self, stage: NodeJoinStage) -> io::Result<()> {
        OwnerProvisioner::record_node_join_stage(self, stage)
    }
    fn node_join_stage_recorded(&self, stage: NodeJoinStage) -> io::Result<bool> {
        OwnerProvisioner::node_join_stage_recorded(self, stage)
    }
    async fn check_node_join_local_binding(&self) -> io::Result<()> {
        OwnerProvisioner::check_node_join_local_binding(self).await
    }
}
