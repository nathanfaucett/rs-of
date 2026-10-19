#![forbid(unsafe_code)]

#[cfg(feature = "cli")]
mod cli;
mod config;
#[cfg(unix)]
mod independent_reset;
#[cfg(unix)]
pub use independent_reset::reset_independent;
#[cfg(all(unix, feature = "cli"))]
mod startup;
#[cfg(all(unix, feature = "cli"))]
pub use startup::prepare_setup;
#[cfg(feature = "network")]
mod copy_retirement;
#[cfg(feature = "network")]
mod data_protocol;
#[cfg(feature = "network")]
mod database_protocol;
#[cfg(feature = "network")]
mod database_transport;
#[cfg(all(test, feature = "network"))]
mod execution_test_support;
#[cfg(feature = "network")]
mod filesystem_transport;
mod idp_client;
#[cfg(feature = "network")]
mod local_replication_authorizer;
mod management_client;
#[cfg(feature = "network")]
mod replication_runtime;
mod router;
#[cfg(feature = "network")]
mod runtime;
#[cfg(feature = "network")]
mod storage_protocol;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::AppConfig;
#[cfg(feature = "network")]
pub use copy_retirement::StorageCopyRetirement;
#[cfg(feature = "network")]
pub use data_protocol::DataProtocolHandler;
#[cfg(feature = "network")]
pub use database_transport::DatabaseTransport;
pub use idp_client::{IdpClient, IdpIntrospectionError};
#[cfg(feature = "network")]
pub use local_replication_authorizer::{DeviceReplicationPolicy, LocalReplicationAuthorizer};
pub use management_client::ManagementClient;
#[cfg(feature = "network")]
pub use replication_runtime::StorageReplicationRuntime;
pub use router::{
    RouterState, StorageSocketAccess, StorageSocketAuthorizationError, StorageSocketAuthorizer,
    openapi_router, resource_router, storage_router,
};
pub use router::{
    StorageAuthorization, StorageAuthorizationError, authorize_storage_token,
    scoped_file_system_socket_authorizer, scoped_file_system_socket_router,
};
#[cfg(feature = "network")]
pub use runtime::{StorageRuntime, build_http_runtime, build_runtime};

#[cfg(all(unix, feature = "network"))]
mod independent_create;
#[cfg(all(unix, feature = "network"))]
pub use independent_create::serve_independent_create;
