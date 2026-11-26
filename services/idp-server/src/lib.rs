#![forbid(unsafe_code)]
#![cfg_attr(test, recursion_limit = "256")]

// Shorter namespace aliases for use within this crate
pub use device_service as device;
pub use device_service::db_kv;
pub use device_service::db_sql;
pub use of_model as model;

#[cfg(feature = "cli")]
pub mod cli;
mod config;
#[cfg(all(unix, feature = "cli"))]
mod startup;
#[cfg(all(unix, feature = "cli"))]
pub use startup::prepare_setup;

mod management_owner;
mod management_permission_client;

mod local_seed;
mod node_provisioning;
#[cfg(test)]
mod node_provisioning_tests;

#[cfg(test)]
mod owner_test_fixture;
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
mod independent_reset;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::{AppConfig, ManagementPermissionConfig};

#[cfg(all(unix, feature = "independent"))]
pub use independent_create::{run_owner_setup, serve_independent_create};
#[cfg(all(unix, feature = "independent"))]
pub use independent_reset::reset_independent;
pub use management_owner::{active_management_user, authorize_management_bearer};
pub use management_permission_client::ManagementPermissionClient;

pub use node_provisioning::validate_metadata_root;
pub use provisioning::{OwnerProvisioner, ensure_initial_signer, open_local_identity_database};
pub use router::{
    RouterState, authorize_bearer, openapi_document, openapi_router, public_metadata_router,
};
pub use runtime::{
    IdpRuntime, build_local_router, build_oauth_runtime, build_runtime, build_state_with_keys,
    identity_client,
};

pub mod local_provisioning;
pub use server_common::LocalTls;
