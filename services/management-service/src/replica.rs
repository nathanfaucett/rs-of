//! Replica modules for Management service
//!
//! This module re-exports the database repositories from the `ofdb` module
//! to provide a stable `management_service::replica` path for downstream crates.

pub use crate::ofdb::DbPermissionRepo;
pub use crate::ofdb::DbRoleRepo;
