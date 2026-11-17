#[cfg(feature = "ofdb")]
use std::{future::Future, io, pin::Pin, sync::Arc};

use idp_service::oauth2::NodeSignerBinding;
use iroh::EndpointId;
use of_model::idp::contract::jwk_public::JwkPublic;
use ofdb_sql::Database;

use of_model::node_join::NodeJoinApproval;

use of_model::node_join::NodeJoinRequest;

#[derive(Clone, Copy)]
pub enum NodeJoinStage {
    Idp,
    Management,
    PublicReady,
}

/// Retained identity-owner state used by Device admission and public bootstrap.
/// Implementations must verify the retained private signer and exact durable anchor.
pub trait NodeJoinOwner: Send + Sync {
    fn node_join_database(&self) -> io::Result<Arc<Database>>;
    fn prepare_node_join(
        &self,
        binding: &NodeSignerBinding,
        name: &str,
        peer: EndpointId,
    ) -> io::Result<NodeJoinRequest>;
    fn record_node_join_approval(&self, approval: &NodeJoinApproval) -> io::Result<()>;
    fn node_join_request(&self) -> io::Result<Option<NodeJoinRequest>>;
    fn node_join_approval(&self) -> io::Result<Option<NodeJoinApproval>>;
    fn node_join_anchor(&self) -> io::Result<NodeJoinApproval>;
    fn record_node_join_stage(&self, stage: NodeJoinStage) -> io::Result<()>;
    fn node_join_stage_recorded(&self, stage: NodeJoinStage) -> io::Result<bool>;
    fn check_node_join_local_binding(&self) -> impl Future<Output = io::Result<()>> + Send;
    fn verify_node_signer(
        &self,
        binding: &NodeSignerBinding,
    ) -> Pin<Box<dyn Future<Output = io::Result<JwkPublic>> + Send>>;
}

pub fn check_bootstrap_endpoint(
    endpoint: EndpointId,
    binding: &NodeSignerBinding,
) -> io::Result<()> {
    if binding.endpoint_id != endpoint.to_string() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "bootstrap signer must bind the shared local endpoint",
        ));
    }
    Ok(())
}
