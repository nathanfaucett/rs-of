#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub use of_model::device::contract::DeviceState;
pub use of_model::device::contract::{
    ApprovedDeviceEndpoints, DeviceApprovalRequest, DeviceEndpointIdentity, DeviceEnrollment,
    DeviceEnrollmentRequest, DeviceInfo, DevicePairingApprovalPayload,
    DevicePairingApprovalRequest, DevicePairingRequest, DeviceSelfRevocationRequest,
    DeviceTransferRequest, PairingAcceptance, TrustedDevice, UpdateDeviceRequest,
};
pub use of_model::device::device::Device;

#[cfg(feature = "ofdb")]
pub mod ofdb;

#[cfg(feature = "std")]
mod device_enrollment;
#[cfg(feature = "std")]
mod device_identity;
mod device_repo;
#[cfg(feature = "std")]
mod hosted_control_plane;
#[cfg(feature = "std")]
mod pairing_acceptance;

#[cfg(feature = "std")]
pub use device_enrollment::DeviceEnrollmentService;
#[cfg(feature = "std")]
pub use device_identity::{
    DeviceIdentity, delete as delete_device_identity,
    identity_from_server as device_identity_from_server, open as open_device_identity,
    open_with_allowlist as open_device_identity_with_allowlist,
};
pub use device_repo::DeviceRepo;
#[cfg(feature = "std")]
pub use hosted_control_plane::HostedControlPlane;
pub use of_model::device_error::{DeviceError, DeviceResult};
#[cfg(feature = "std")]
pub use pairing_acceptance::{
    PairingAcceptanceController, PairingAcceptanceControllerSlot, TimedPairingAcceptanceController,
};

#[cfg(all(feature = "ofdb", unix))]
pub mod independent_join;
#[cfg(all(feature = "ofdb", unix))]
pub mod join_history;
#[cfg(feature = "ofdb")]
pub mod security_candidates;
#[cfg(feature = "ofdb")]
mod sync_validation_direction;

#[cfg(feature = "ofdb")]
pub mod security_policy;
mod security_sync;
pub use security_sync::{PUBLIC_DEVICE_TABLES, permits_device_table, public_device_columns};

#[cfg(feature = "std")]
pub mod hosted_join;
#[cfg(feature = "ofdb")]
pub mod issuer_discovery;

#[cfg(all(test, feature = "ofdb"))]
mod security_protocol_contract_tests;

#[cfg(feature = "ofdb")]
pub mod security_protocol;

#[cfg(feature = "ofdb")]
pub mod node_join_owner;

#[cfg(feature = "ofdb")]
pub mod join_admission;
