#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
mod cli;
mod config;
mod runtime;
#[cfg(unix)]
mod setup_host;

pub use server_common::LocalTls;
#[cfg(feature = "cli")]
pub use cli::run;
pub use config::UnifiedConfig;
pub use runtime::compose_router;
#[cfg(unix)]
pub use setup_host::SetupHost;
