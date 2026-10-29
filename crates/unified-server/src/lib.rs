#![forbid(unsafe_code)]
#![recursion_limit = "256"]

mod bootstrap_verification;
#[cfg(feature = "cli")]
mod cli;
mod config;
mod endpoint;
mod endpoint_secret;
#[cfg(all(test, target_os = "linux"))]
mod hosted_setup_integration_tests;
#[cfg(all(test, unix))]
mod http_server_tests;
#[cfg(all(test, unix))]
mod local_provisioning_tests;
mod runtime;
mod setup_host;
#[cfg(all(test, unix))]
mod setup_host_tests;
#[cfg(unix)]
mod setup_runtime;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::UnifiedConfig;
pub use device_service::security_protocol::{SecurityContext, SecurityProtocol};
pub use endpoint::UnifiedEndpoint;
pub use runtime::{RuntimeOwners, UnifiedRuntime, compose_protocol_router, compose_router};
#[cfg(unix)]
pub use setup_host::SetupHost;
#[cfg(unix)]
pub use setup_runtime::ApiSetupHost;

#[cfg(all(test, unix))]
mod setup_runtime_tests;

mod bootstrap_config;
mod credential_bootstrap;
mod local_join;
mod local_provisioning;
pub use api::LocalTls;
pub use bootstrap_config::{BootstrapConfig, BootstrapPurpose};
pub use credential_bootstrap::CredentialBootstrap;
pub use endpoint::discover_installation_issuer;
pub use local_join::LocalJoin;
pub use local_provisioning::LocalProvisioner;
#[cfg(test)]
mod bootstrap_config_tests;
#[cfg(all(test, not(unix)))]
mod unsupported_bootstrap_tests;
