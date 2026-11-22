#![forbid(unsafe_code)]

// Shorter namespace aliases for use within this crate
pub use device_service as device;
pub use device_service::db_kv;
pub use device_service::db_sql;
pub use of_model as model;

#[cfg(feature = "cli")]
pub mod cli;
mod config;
#[cfg(all(unix, feature = "independent"))]
mod independent_create;
#[cfg(all(unix, feature = "independent"))]
mod independent_reset;
mod provisioning;
mod router;
mod runtime;
#[cfg(all(unix, feature = "independent"))]
mod startup;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::AppConfig;
#[cfg(all(unix, feature = "independent"))]
pub use independent_create::{run_owner_setup, serve_independent_create};
#[cfg(all(unix, feature = "independent"))]
pub use independent_reset::reset_independent;
pub use management_service::MANAGEMENT_APPLICATION_URI;
pub use provisioning::{
    open_device_database, open_local_device_database, provision_initial_device,
};
pub use router::{RouterState, openapi_document, openapi_router, public_metadata_router};
pub use runtime::{build_local_router, build_router};
#[cfg(all(unix, feature = "independent"))]
pub use startup::prepare_setup;

mod authority;
pub use authority::DeviceAuthority;

mod database_client;
pub use database_client::{device_client, identity_client};
