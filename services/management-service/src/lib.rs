#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;

// Shorter namespace aliases for use within this crate
#[cfg(feature = "std")]
pub use of_model as model;
#[cfg(all(feature = "ofdb", feature = "redb"))]
pub use ofdb_kv as db_kv;
#[cfg(feature = "ofdb")]
pub use ofdb_sql as db_sql;

#[cfg(feature = "ofdb")]
mod local_permissions;
#[cfg(feature = "ofdb")]
pub mod ofdb;
pub mod permission_repo;
#[cfg(feature = "ofdb")]
pub mod replica;
pub mod role_repo;
pub mod service;

#[cfg(feature = "ofdb")]
pub use local_permissions::LocalPermissions;
pub use of_model::management_error::{ManagementError, ManagementResult};
#[cfg(feature = "ofdb")]
pub use ofdb::provision_initial_administrator;
pub use permission_repo::PermissionRepo;
pub use role_repo::RoleRepo;
pub use service::{INITIAL_ADMINISTRATOR_ACTIONS, MANAGEMENT_APPLICATION_URI, ManagementService};
