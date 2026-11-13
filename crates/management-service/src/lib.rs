#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
#[cfg(feature = "replica")]
mod local_permissions;
mod permission_repo;
#[cfg(feature = "replica")]
pub mod replica;
mod role_repo;
mod service;

#[cfg(feature = "replica")]
pub use local_permissions::LocalPermissions;
pub use of_model::management_error::{ManagementError, ManagementResult};
pub use of_model::sync_selection::{ManagementSyncSelection, SyncSelectionRepo};
pub use permission_repo::PermissionRepo;
pub use role_repo::RoleRepo;
pub use service::{INITIAL_ADMINISTRATOR_ACTIONS, MANAGEMENT_APPLICATION_URI, ManagementService};

pub use of_model::management::{SyncSelection, SyncSelectionRequest, SyncSelectionResponse};
