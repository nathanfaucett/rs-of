#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
pub mod cli;
mod config;
mod runtime;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::{SeedConfig, SeedUser, UnifiedConfig};
pub use runtime::{compose_local_router, compose_router, serve};
