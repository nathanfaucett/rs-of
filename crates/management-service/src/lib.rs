#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
mod error;
#[cfg(feature = "replica")]
mod local_permissions;
mod permission_repo;
#[cfg(feature = "replica")]
pub mod replica;
mod role_repo;
mod service;
pub use error::{ManagementError, ManagementResult};
#[cfg(feature = "replica")]
pub use local_permissions::LocalPermissions;
pub use permission_repo::PermissionRepo;
pub use role_repo::RoleRepo;
pub use service::{INITIAL_ADMINISTRATOR_ACTIONS, MANAGEMENT_APPLICATION_URI, ManagementService};
