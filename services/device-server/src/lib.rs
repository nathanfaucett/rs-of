#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
pub mod cli;
mod config;
mod provisioning;
mod router;
mod runtime;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::AppConfig;
pub use management_service::MANAGEMENT_APPLICATION_URI;
pub use provisioning::{
    open_device_database, open_local_device_database, provision_initial_device,
};
pub use router::{RouterState, openapi_document, openapi_router, public_metadata_router};
pub use runtime::{build_local_router, build_router};

mod node_join_source;
pub use node_join_source::{NodeJoinReadiness, NodeJoinSource, node_join_openapi};

mod authority;
pub use authority::DeviceAuthority;

mod database_client;
pub use database_client::{device_client, identity_client};
