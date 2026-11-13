#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
mod cli;
mod config;
mod runtime;
mod tls;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::{SeedConfig, SeedUser, TlsConfig, UnifiedConfig};
pub use runtime::{compose_local_router, compose_router, serve};
