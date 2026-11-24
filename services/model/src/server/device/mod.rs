#[allow(
    clippy::module_inception,
    reason = "preserve the existing public server::device::device module path"
)]
pub mod device;
pub mod device_restriction;
pub mod device_selection;
