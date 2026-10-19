#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
mod cli;
mod config;
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
pub use provisioning::{open_database, provision_initial_administrator, provision_initial_device};
pub use router::{RouterState, openapi_router};
pub use runtime::{build_router, replication_policy};

#[cfg(unix)]
mod independent_create;
#[cfg(unix)]
pub use independent_create::serve_independent_create;
