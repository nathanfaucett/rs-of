#![forbid(unsafe_code)]
extern crate ofdb_sync;

// Shorter namespace aliases for use within this crate
pub use file_system as fs;
pub use of_model as model;
pub use ofdb_kv as db_kv;
pub use ofdb_sql as db_sql;

#[cfg(feature = "cli")]
pub mod cli;
mod config;
#[cfg(feature = "network")]
mod copy_retirement;
#[cfg(feature = "network")]
mod data_protocol;
#[cfg(feature = "network")]
mod database_protocol;
#[cfg(feature = "network")]
mod database_sessions;
#[cfg(feature = "network")]
mod device_client;
#[cfg(all(test, feature = "network"))]
mod execution_test_support;
#[cfg(feature = "network")]
mod filesystem_transport;
mod idp_client;
#[cfg(all(unix, feature = "independent"))]
mod independent_create;
#[cfg(all(unix, feature = "independent"))]
mod independent_reset;
#[cfg(feature = "network")]
mod local_replication_authorizer;
mod management_client;
#[cfg(feature = "network")]
mod provisioning;
#[cfg(feature = "network")]
mod replication_admission;
#[cfg(feature = "network")]
mod replication_runtime;
mod router;
#[cfg(feature = "network")]
mod runtime;
#[cfg(all(unix, feature = "independent"))]
mod startup;
#[cfg(feature = "network")]
mod storage_protocol;
#[cfg(feature = "sync")]
mod sync;

#[cfg(feature = "cli")]
pub use cli::run;
pub use config::AppConfig;
#[cfg(feature = "network")]
pub use copy_retirement::StorageCopyRetirement;
#[cfg(feature = "network")]
pub use data_protocol::DataProtocolHandler;
#[cfg(feature = "network")]
pub use db::DatabaseTransport;
pub use device_client::DeviceClient;
pub use idp_client::{IdpClient, IdpIntrospectionError};
#[cfg(all(unix, feature = "independent"))]
pub use independent_create::serve_independent_create;
#[cfg(feature = "network")]
pub use local_replication_authorizer::{DeviceReplicationPolicy, LocalReplicationAuthorizer};
pub use management_client::ManagementClient;
#[cfg(feature = "network")]
pub use replication_runtime::StorageReplicationRuntime;
pub use router::{
    RouterState, StorageSocketAccess, StorageSocketAuthorizationError, StorageSocketAuthorizer,
    resource_router, storage_router,
};
pub use router::{
    StorageAuthorization, StorageAuthorizationError, authorize_storage_token,
    scoped_file_system_socket_authorizer, scoped_file_system_socket_router,
};
pub use router::{openapi_document, openapi_router, public_metadata_router};
#[cfg(feature = "network")]
pub use runtime::{StorageRuntime, build_http_runtime, build_local_runtime, build_runtime};
#[cfg(all(unix, feature = "independent"))]
pub use startup::prepare_setup;
#[cfg(feature = "sync")]
pub use sync::{SyncState, sync_router};
