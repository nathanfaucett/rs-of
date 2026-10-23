#![forbid(unsafe_code)]
#![recursion_limit = "256"]

mod bootstrap_verification;
#[cfg(feature = "cli")]
mod cli;
mod config;
mod endpoint;
#[cfg(all(test, target_os = "linux"))]
mod hosted_setup_integration_tests;
#[cfg(all(test, unix))]
mod http_server_tests;
#[cfg(all(test, unix))]
mod local_provisioning_tests;
#[cfg(test)]
mod native_api_tests;
#[cfg(test)]
mod node_join_bootstrap_tests;
#[cfg(test)]
mod permission_http_tests;
mod runtime;
#[cfg(test)]
mod security_history_tests;
mod security_sync;
#[cfg(test)]
mod security_sync_tests;
mod setup_host;
#[cfg(all(test, unix))]
mod setup_host_tests;
#[cfg(unix)]
mod setup_runtime;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::UnifiedConfig;
pub use endpoint::UnifiedEndpoint;
pub use runtime::{RuntimeOwners, UnifiedRuntime, compose_protocol_router, compose_router};
pub use security_sync::{SecurityContext, SecurityProtocol};
#[cfg(unix)]
pub use setup_host::SetupHost;
#[cfg(unix)]
pub use setup_runtime::ApiSetupHost;

#[cfg(test)]
mod generated_transport_tests;

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

#[cfg(all(test, unix))]
mod owner_composition_tests;

#[cfg(test)]
mod device_authority_tests;

#[cfg(test)]
mod node_bootstrap_interruption_tests;
