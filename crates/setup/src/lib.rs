#[cfg(unix)]
mod canonical_openapi;
#[cfg(unix)]
pub mod independent_create;
#[cfg(unix)]
mod independent_exchange;
#[cfg(all(test, unix))]
mod independent_exchange_lifecycle_tests;
#[cfg(unix)]
mod independent_history;
#[cfg(unix)]
mod independent_openapi;
#[cfg(unix)]
mod independent_operation;
#[cfg(unix)]
mod independent_peer;
#[cfg(unix)]
mod independent_reset;
#[cfg(unix)]
mod independent_setup;
mod local_tls;
#[cfg(unix)]
mod setup_admission;
pub mod setup_files;
#[cfg(unix)]
mod setup_https_fixture;
#[cfg(unix)]
mod setup_install;
#[cfg(unix)]
mod setup_routes;
#[cfg(unix)]
mod setup_scope;
#[cfg(unix)]
mod setup_server;

#[cfg(unix)]
pub use canonical_openapi::{canonical_openapi, canonical_openapi_router};
#[cfg(unix)]
pub use independent_create::{
    IndependentCreateCommand, IndependentCreateControl, IndependentCreateHost,
    IndependentRegistrar, IndependentSecretLoader,
    setup_routes,
};
#[cfg(unix)]
pub use independent_exchange::{random_capability, routes};
#[cfg(unix)]
pub use independent_operation::{
    IndependentCreateIntent, IndependentCreateRequest, IndependentOwnerPublic,
};
#[cfg(unix)]
pub use independent_peer::{IndependentPeer, exact_https};
#[cfg(unix)]
pub use independent_reset::{IndependentReset, uninstall_local_ca};
#[cfg(unix)]
pub use independent_setup::{IndependentSetupHost, HostSettings};
pub use local_tls::{LocalTls, LocalTlsConfig, TlsListener};
#[cfg(unix)]
pub use setup_admission::{
    SetupAdmission, SetupAdmissionError, SetupAdmissionHistory, setup_admission_middleware,
};
pub use setup_files::{
    absolute, create_private_directory, present, private_options, read_private, require_absent,
    require_private, sync_ancestry,
};
#[cfg(unix)]
pub use setup_routes::{setup_origin, setup_origin_guard, setup_router};
#[cfg(unix)]
pub use setup_scope::SetupScope;
#[cfg(unix)]
pub use setup_server::serve_setup;
#[cfg(all(unix, feature = "test-support"))]
pub use setup_server::serve_setup_verified;

#[cfg(all(test, unix))]
mod independent_setup_tests;
#[cfg(all(test, unix))]
mod independent_exchange_lifecycle_tests;
#[cfg(all(test, target_os = "linux"))]
mod setup_install_tests;
#[cfg(all(test, unix))]
mod setup_admission_tests;
#[cfg(all(test, unix))]
mod setup_server_teardown_tests;
