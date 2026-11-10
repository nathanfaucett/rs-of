mod config;
mod portless;
pub use portless::{bind_addr, public_base_url};
mod openapi;
pub use openapi::SecurityAddon;
mod serve;
pub use serve::{serve, serve_listener};
pub use config::{Environment, ServerConfig};

// Re-export all setup types from setup crate
#[cfg(unix)]
pub use setup::{
    canonical_openapi, canonical_openapi_router,
    IndependentCreateCommand, IndependentCreateControl, IndependentCreateHost,
    IndependentRegistrar, IndependentSecretLoader,
    IndependentCreateIntent, IndependentCreateRequest, IndependentOwnerPublic,
    IndependentPeer, exact_https,
    IndependentReset, uninstall_local_ca,
    IndependentSetupHost, HostSettings,
    LocalTls, LocalTlsConfig, TlsListener,
    SetupAdmission, SetupAdmissionError, SetupAdmissionHistory, setup_admission_middleware,
    SetupScope, setup_origin, setup_origin_guard, setup_router,
    serve_setup,
    absolute, create_private_directory, present, private_options, read_private,
    require_absent, require_private, sync_ancestry,
    random_capability, routes,
    setup_files,
};
#[cfg(all(unix, feature = "test-support"))]
pub use setup::serve_setup_verified;
