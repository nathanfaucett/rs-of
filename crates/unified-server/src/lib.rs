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
mod independent_permission_http_tests;
#[cfg(all(test, unix))]
mod local_provisioning_tests;
#[cfg(test)]
mod native_api_tests;
#[cfg(test)]
mod node_bootstrap_owner_tests;
#[cfg(test)]
mod node_join_bootstrap_tests;
#[cfg(test)]
mod permission_http_tests;
mod protocol;
mod router;
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
pub use protocol::compose_protocol_router;
pub use router::compose_router;
pub use runtime::{RuntimeOwners, UnifiedRuntime};
pub use security_sync::{SecurityContext, SecurityProtocol};
#[cfg(unix)]
pub use setup_host::SetupHost;
#[cfg(unix)]
pub use setup_runtime::ApiSetupHost;

#[cfg(all(test, target_os = "linux"))]
mod independent_create_tests;
#[cfg(all(test, target_os = "linux"))]
mod independent_join_tests;
#[cfg(all(test, target_os = "linux"))]
mod independent_operation_tests;
#[cfg(all(test, target_os = "linux"))]
mod independent_reset_tests;
#[cfg(all(test, target_os = "linux"))]
mod independent_transfer_tests;

#[cfg(test)]
mod generated_transport_tests;

mod owner_health;

#[cfg(all(test, unix))]
mod setup_runtime_tests;

mod bootstrap_adapter;
pub use api::LocalTls;
pub use bootstrap_adapter::discover_installation_issuer;
pub use bootstrap_adapter::{
    BootstrapConfig, CredentialBootstrap, LocalJoin, LocalProvisioner, UnifiedBootstrapHost,
};
pub use idp_server::BootstrapPurpose;
#[cfg(test)]
mod bootstrap_config_tests;
#[cfg(all(test, not(unix)))]
mod unsupported_bootstrap_tests;
