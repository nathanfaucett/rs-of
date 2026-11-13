#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub use of_model::device::{
    ApprovedDeviceEndpoints, Device, DeviceApprovalRequest, DeviceEndpointIdentity,
    DeviceEnrollment, DeviceEnrollmentRequest, DeviceInfo, DevicePairingApprovalPayload,
    DevicePairingApprovalRequest, DevicePairingRequest, DeviceSelfRevocationRequest, DeviceState,
    DeviceTransferRequest, PairingAcceptance, TrustedDevice, UpdateDeviceRequest,
    device_pairing_approval_payload, device_self_revocation_payload,
};

#[cfg(feature = "replica")]
pub mod replica;

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

#[cfg(all(feature = "independent", unix))]
pub mod independent_join;
#[cfg(all(feature = "independent", unix))]
pub mod join_history;
#[cfg(feature = "replica")]
pub mod security_candidates;
#[cfg(feature = "replica")]
mod sync_validation_direction;

#[cfg(feature = "replica")]
pub mod security_policy;
mod security_sync;
pub use security_sync::{PUBLIC_DEVICE_TABLES, permits_device_table, public_device_columns};

#[cfg(feature = "std")]
pub mod hosted_join;
#[cfg(feature = "replica")]
pub mod issuer_discovery;

#[cfg(all(test, feature = "replica"))]
mod security_protocol_contract_tests;

#[cfg(feature = "replica")]
pub mod security_protocol;

#[cfg(feature = "replica")]
pub mod node_join_owner;

#[cfg(feature = "replica")]
pub mod join_admission;
