#![forbid(unsafe_code)]

// Shorter namespace aliases for use within this crate
pub use of_model as model;
pub use ofdb_sql as db_sql;

#[cfg(feature = "cli")]
pub mod cli;
mod config;
#[cfg(unix)]
mod independent_reset;
mod provisioning;
mod router;
mod runtime;
#[cfg(feature = "cli")]
pub use cli::run;
pub use config::AppConfig;
#[cfg(unix)]
pub use independent_reset::reset_independent;
pub use management_service::MANAGEMENT_APPLICATION_URI;
pub use provisioning::{
    InitialAdministratorGrant, identity_client, provision_initial_administrator,
};
pub use router::{RouterState, openapi_document, openapi_router, public_metadata_router};
pub use runtime::{build_local_router, build_router};
