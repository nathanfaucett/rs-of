pub mod args;
#[cfg(feature = "completions")]
mod completions;
pub mod shutdown_signal;

pub use args::{CliArgs, CliServerCommand, CliServerServe};
#[cfg(feature = "completions")]
pub use clap_complete::Shell;
#[cfg(feature = "completions")]
pub use completions::run_completions;

pub use shutdown_signal::shutdown_signal;
