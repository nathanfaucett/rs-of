extern crate alloc;

pub mod contract;
#[allow(
    clippy::module_inception,
    reason = "preserve the existing public device::device module path"
)]
pub mod device;
pub mod device_state_sql;
pub mod selection_policy;
