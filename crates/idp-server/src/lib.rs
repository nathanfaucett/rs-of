#![forbid(unsafe_code)]
#![cfg_attr(test, recursion_limit = "256")]

#[cfg(feature = "cli")]
mod cli;
mod config;
#[cfg(all(unix, feature = "cli"))]
mod startup;
#[cfg(all(unix, feature = "cli"))]
pub use startup::prepare_setup;

mod bootstrap;

mod management_owner;
mod management_permission_client;

mod node_join;
#[cfg(test)]
mod node_join_review_tests;
#[cfg(test)]
mod node_join_tests;
mod node_provisioning;
#[cfg(test)]
mod node_provisioning_tests;
#[cfg(test)]
mod permission_evaluation_tests;
mod provisioning;
mod router;
mod runtime;

mod storage_owner;
#[cfg(feature = "cli")]
mod unavailable_data_protocol;

pub use bootstrap::{BOOTSTRAP_ALPN, BootstrapProtocolHandler};
#[cfg(feature = "cli")]
pub use cli::run;
pub use config::{AppConfig, ManagementPermissionConfig, PairingConfig};
pub use management_owner::{active_management_user, authorize_management_bearer};
pub use management_permission_client::ManagementPermissionClient;

pub use node_join::{NodeJoinApproval, NodeJoinRequest, NodeJoinStage, bind_node_join_endpoint};
pub use provisioning::OwnerProvisioner;
pub use router::{RouterState, authorize_bearer, openapi_router};
pub(crate) use router::{authorize_bearer_any_principal, authorize_bearer_client};
pub use runtime::{IdpRuntime, build_runtime};
