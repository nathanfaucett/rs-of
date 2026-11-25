#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
pub mod cli;
mod config;
#[cfg(all(unix, feature = "cli", feature = "independent-contracts"))]
mod first_run;
#[cfg(all(unix, feature = "cli", feature = "independent-contracts"))]
mod node_setup;
mod runtime;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::{SeedConfig, SeedUser, UnifiedConfig};
#[cfg(all(unix, feature = "cli", feature = "independent-contracts"))]
pub use node_setup::node_setup_openapi;
pub use runtime::{compose_local_router, compose_router, serve};
