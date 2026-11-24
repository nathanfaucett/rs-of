#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
pub mod cli;
mod config;
#[cfg(all(unix, feature = "cli", feature = "independent-contracts"))]
mod first_run;
mod runtime;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::{SeedConfig, SeedUser, UnifiedConfig};
pub use runtime::{compose_local_router, compose_router, serve};
