pub(crate) mod device;
pub(crate) mod device_restriction;
pub(crate) mod device_selection;
pub(crate) mod device_self_revocation;
pub(crate) mod devices;
pub(crate) mod health;

pub(crate) mod replication;
pub(crate) mod version;

#[cfg(test)]
mod device_authority_tests;

#[cfg(test)]
mod permission_boundary_tests;
