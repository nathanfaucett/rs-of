mod canonical_openapi;
pub use canonical_openapi::{canonical_openapi, canonical_openapi_router};
mod config;
#[cfg(unix)]
mod independent_reset;
#[cfg(unix)]
pub use independent_reset::{IndependentReset, uninstall_local_ca};
#[cfg(unix)]
mod independent_create;
#[cfg(unix)]
mod independent_exchange;
#[cfg(unix)]
mod independent_history;
#[cfg(unix)]
mod independent_openapi;
#[cfg(unix)]
mod independent_operation;
#[cfg(unix)]
pub use independent_openapi::independent_setup_openapi;
#[cfg(unix)]
mod independent_peer;
#[cfg(unix)]
pub use independent_create::{
    IndependentCreateCommand, IndependentCreateControl, IndependentCreateHost,
    IndependentRegistrar, IndependentSecretLoader,
};
#[cfg(unix)]
pub use independent_operation::{
    IndependentCreateIntent, IndependentCreateRequest, IndependentOwnerPublic,
};
#[cfg(unix)]
pub use independent_peer::{IndependentPeer, exact_https};
#[cfg(unix)]
mod independent_setup;
#[cfg(all(test, unix))]
mod independent_setup_tests;
mod local_tls;
mod openapi;
pub mod setup_files;
#[cfg(unix)]
mod setup_install;
#[cfg(all(test, target_os = "linux"))]
mod setup_install_tests;
#[cfg(unix)]
mod setup_routes;
#[cfg(unix)]
mod setup_scope;
#[cfg(unix)]
mod setup_server;

#[cfg(unix)]
pub use independent_setup::IndependentSetupHost;
pub use local_tls::{LocalTls, LocalTlsConfig, TlsListener};
#[cfg(unix)]
pub use setup_routes::{setup_origin, setup_origin_guard, setup_router};
#[cfg(unix)]
pub use setup_scope::SetupScope;
#[cfg(unix)]
pub use setup_server::serve_setup;
#[cfg(all(unix, feature = "test-support"))]
pub use setup_server::serve_setup_verified;
mod serve;
#[cfg(unix)]
mod setup_admission;
#[cfg(all(test, unix))]
mod setup_admission_tests;

pub use config::{Environment, ServerConfig};
pub use openapi::SecurityAddon;
pub use serve::{serve, serve_listener};
#[cfg(unix)]
pub use setup_admission::{SetupAdmission, SetupAdmissionError, setup_admission_middleware};

#[cfg(unix)]
pub use setup_routes::owner_setup_openapi;
