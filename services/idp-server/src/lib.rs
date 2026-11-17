#![forbid(unsafe_code)]
#![cfg_attr(test, recursion_limit = "256")]

#[cfg(feature = "cli")]
pub mod cli;
mod config;
#[cfg(all(unix, feature = "cli"))]
mod startup;
#[cfg(all(unix, feature = "cli"))]
pub use startup::prepare_setup;

mod bootstrap;

mod management_owner;
mod management_permission_client;

mod local_seed;
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

#[cfg(all(unix, feature = "independent"))]
mod independent_create;
#[cfg(all(unix, feature = "independent"))]
mod independent_identity;
#[cfg(all(unix, feature = "independent"))]
mod independent_join;
#[cfg(all(unix, feature = "independent"))]
mod independent_reset;

pub use bootstrap::{BOOTSTRAP_ALPN, BootstrapProtocolHandler};
#[cfg(feature = "cli")]
pub use cli::run;
pub use config::{AppConfig, ManagementPermissionConfig, PairingConfig};
pub use management_owner::{active_management_user, authorize_management_bearer};
pub use management_permission_client::ManagementPermissionClient;

pub use device_service::join_admission::bind_node_join_endpoint;
pub use device_service::node_join_owner::NodeJoinStage;
pub use provisioning::{OwnerProvisioner, open_local_identity_database};
pub use router::{
    RouterState, authorize_bearer, openapi_document, openapi_router, public_metadata_router,
};
pub(crate) use router::{authorize_bearer_any_principal, authorize_bearer_client};
pub use runtime::{
    IdpRuntime, build_local_router, build_oauth_runtime, build_runtime, identity_client,
};

pub mod issuer_discovery;
pub mod local_provisioning;
pub use issuer_discovery::discover_installation_issuer;
pub use server_common::LocalTls;
