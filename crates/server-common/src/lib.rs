mod portless;
pub use portless::{bind_addr, public_base_url};
mod openapi;
pub use openapi::SecurityAddon;
mod canonical_openapi;
mod serve;
pub use canonical_openapi::{canonical_openapi, canonical_openapi_router};
mod http_client;
pub use http_client::verification_client;
pub use of_model::config::{Environment, ServerConfig};
pub use serve::{serve, serve_listener};

#[cfg(all(unix, feature = "test-support"))]
pub use setup::serve_setup_verified;
#[cfg(all(unix, feature = "independent"))]
pub use setup::{
    HostSettings, IndependentCreateCommand, IndependentCreateControl, IndependentCreateHost,
    IndependentCreateIntent, IndependentCreateRequest, IndependentOwnerPublic, IndependentPeer,
    IndependentRegistrar, IndependentReset, IndependentSecretLoader, IndependentSetupHost,
    LocalTls, LocalTlsConfig, SetupAdmission, SetupAdmissionError, SetupAdmissionHistory,
    SetupScope, TlsListener, absolute, create_private_directory, exact_https, present,
    private_options, random_capability, read_private, require_absent, require_private, routes,
    serve_setup, setup_admission_middleware, setup_files, setup_origin, setup_origin_guard,
    setup_router, sync_ancestry, uninstall_local_ca,
};

#[cfg(all(unix, feature = "test-support"))]
pub mod setup_https_fixture;

#[cfg(all(unix, feature = "independent"))]
pub use setup::owner_setup_openapi;
