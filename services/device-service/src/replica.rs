//! Replica modules for Device service
//!
//! This module re-exports the database repositories from the `ofdb` module
//! to provide a stable `device_service::replica` path for downstream crates.

#[cfg(feature = "ofdb")]
pub use crate::ofdb::DbDeviceRepo;
#[cfg(feature = "ofdb")]
pub use crate::ofdb::DbSelectionPolicyRepo;
#[cfg(feature = "ofdb")]
pub use crate::ofdb::{up, validate_device};
