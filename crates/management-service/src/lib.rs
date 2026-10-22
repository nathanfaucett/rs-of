#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod contract;
mod device;
mod device_state_sql;

pub use contract::{
    ApprovedDeviceEndpoints, DeviceApprovalRequest, DeviceEndpointIdentity, DeviceEnrollment,
    DeviceEnrollmentRequest, DeviceInfo, DevicePairingApprovalPayload,
    DevicePairingApprovalRequest, DevicePairingRequest, DeviceSelfRevocationRequest, DeviceState,
    DeviceTransferRequest, PairingAcceptance, TrustedDevice, UpdateDeviceRequest,
    device_pairing_approval_payload, device_self_revocation_payload,
};
pub use device::Device;

#[cfg(feature = "replica")]
pub mod replica;

#[cfg(feature = "std")]
mod device_enrollment;
#[cfg(feature = "std")]
mod device_identity;
mod device_repo;
mod error;
#[cfg(feature = "std")]
mod hosted_control_plane;
#[cfg(feature = "std")]
mod pairing_acceptance;
mod permission_repo;

mod role_repo;
mod service;

#[cfg(feature = "std")]
pub use device_enrollment::DeviceEnrollmentService;
#[cfg(feature = "std")]
pub use device_identity::{
    DeviceIdentity, delete as delete_device_identity,
    identity_from_server as device_identity_from_server, open as open_device_identity,
    open_with_allowlist as open_device_identity_with_allowlist,
};
pub use device_repo::DeviceRepo;
pub use error::{ManagementError, ManagementResult};
#[cfg(feature = "std")]
pub use hosted_control_plane::HostedControlPlane;
#[cfg(feature = "std")]
pub use pairing_acceptance::{
    PairingAcceptanceController, PairingAcceptanceControllerSlot, TimedPairingAcceptanceController,
};
pub use permission_repo::PermissionRepo;

pub use role_repo::RoleRepo;
pub use service::{MANAGEMENT_APPLICATION_URI, ManagementService};

#[cfg(feature = "replica")]
mod local_permissions;
#[cfg(feature = "replica")]
pub use local_permissions::LocalPermissions;

#[cfg(all(feature = "replica", unix))]
pub mod independent_join;
#[cfg(all(feature = "replica", unix))]
pub mod join_history;
#[cfg(feature = "replica")]
pub mod security_candidates;
mod security_sync;
pub use security_sync::{PUBLIC_POLICY_TABLES, permits_policy_table, public_policy_columns};

#[cfg(feature = "std")]
pub mod hosted_join;
#[cfg(feature = "replica")]
pub mod issuer_discovery;
