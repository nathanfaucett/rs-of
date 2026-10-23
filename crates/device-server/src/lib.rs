#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
mod cli;
mod config;
#[cfg(unix)]
mod independent_reset;
#[cfg(unix)]
pub use independent_reset::reset_independent;
#[cfg(all(unix, feature = "cli"))]
mod startup;
#[cfg(all(unix, feature = "cli"))]
pub use startup::prepare_setup;
mod provisioning;
mod router;
mod runtime;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::AppConfig;
pub use management_service::MANAGEMENT_APPLICATION_URI;
pub use provisioning::{open_device_database, provision_initial_device};
pub use router::{RouterState, openapi_document, openapi_router, public_metadata_router};
pub use runtime::build_router;

#[cfg(unix)]
mod independent_create;
#[cfg(unix)]
mod independent_join;
#[cfg(unix)]
pub use independent_create::serve_independent_create;
#[cfg(unix)]
pub use independent_join::{IndependentDeviceRuntime, build_independent_device};

#[cfg(unix)]
pub use independent_join::independent_join_openapi;

mod node_join_source;
pub use node_join_source::{NodeJoinReadiness, NodeJoinSource, node_join_openapi};

mod authority;
pub use authority::DeviceAuthority;

mod database_client;
pub use database_client::{device_client, identity_client};
