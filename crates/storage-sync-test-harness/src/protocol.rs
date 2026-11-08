//! Protocol message types for app-owned data sync.

use std::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a synchronization protocol.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProtocolId {
    /// ofdb SQL sync protocol version 7
    OfdbSqlV7,
    /// ofdb KV sync protocol (future)
    OfdbKvV1,
    /// offs filesystem sync protocol version 1
    OffsFsV1,
    /// Extension point for future protocols
    Other(String),
}

impl ProtocolId {
    pub fn as_str(&self) -> &str {
        match self {
            ProtocolId::OfdbSqlV7 => "ofdb-sql-v7",
            ProtocolId::OfdbKvV1 => "ofdb-kv-v1",
            ProtocolId::OffsFsV1 => "offs-fs-v1",
            ProtocolId::Other(s) => s,
        }
    }
}

impl core::fmt::Display for ProtocolId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Stable identity for a store/replica pair.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoreIdentity {
    /// Stable across restarts, unique per vault
    pub store_id: Uuid,
    /// Unique per device/browser profile
    pub replica_id: Uuid,
    /// Monotonic counter incremented on re-key
    pub creation_epoch: u64,
    /// Schema + vault format hash (32 bytes)
    pub schema_fingerprint: [u8; 32],
}

/// Opaque authentication token from backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthToken(pub Vec<u8>);

/// Synchronization direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum AttachDirection {
    Push = 0,
    Pull = 1,
    Bidirectional = 2,
}

/// Initial hello message for version negotiation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    /// Protocol framing version
    pub protocol_version: u16,
    /// Current store manifest
    pub manifest: Vec<u8>,  // Opaque, protocol-specific
    /// Stable store identity
    pub store_identity: StoreIdentity,
    /// Protocols this peer can speak
    pub supported_protocols: Vec<ProtocolId>,
}

/// Attach request to authorize sync for a specific store.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttachRequest {
    pub store_identity: StoreIdentity,
    pub auth_token: AuthToken,
    pub direction: AttachDirection,
    /// Optional reconnect token for session resumption
    pub reconnect_token: Option<ReconnectToken>,
}

/// Attach response from backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttachResponse {
    pub accepted: bool,
    pub backend_replica_id: Uuid,
    pub session_id: Uuid,
    pub granted_direction: AttachDirection,
    pub max_frame_bytes: u32,
    pub max_session_bytes: u64,
}

/// Reconnect token for session resumption.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReconnectToken {
    pub session_id: Uuid,
    pub last_acknowledged_bytes: u64,
    pub last_acknowledged_units: Vec<SyncUnitId>,
    pub sender_state_hash: [u8; 32],
    pub receiver_state_hash: [u8; 32],
    pub timestamp_ms: u64,
}

/// Unique identifier for a synchronization unit.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SyncUnitId {
    pub protocol_id: ProtocolId,
    /// Protocol-specific unit identifier (e.g., "table:row:change_id")
    pub unit_key: String,
}

/// Selection of what to synchronize.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncSelection {
    /// Protocol-specific selection criteria
    pub criteria: Vec<u8>,  // Opaque, protocol-specific
}

/// Configuration for a sync session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncConfig {
    pub direction: AttachDirection,
    pub max_frame_bytes: u32,
    pub max_session_bytes: u64,
    pub selection: SyncSelection,
}

/// Result of a synchronization session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncResult {
    pub received_units: u64,
    pub sent_units: u64,
    pub received_changes: u64,
    pub sent_changes: u64,
    pub received_snapshots: u64,
    pub sent_snapshots: u64,
    pub acknowledged_bytes: u64,
}

/// Synchronization phase for progress reporting.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SyncPhase {
    Negotiating = 0,
    Attaching = 1,
    ExchangingManifests = 2,
    ExchangingInventories = 3,
    SendingSnapshots = 4,
    ReceivingSnapshots = 5,
    SendingChanges = 6,
    ReceivingChanges = 7,
    ApplyingChanges = 8,
    Finalizing = 9,
}

/// Progress report during synchronization.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProgressReport {
    pub phase: SyncPhase,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub units_completed: u64,
    pub units_total: u64,
    pub eta_ms: Option<u64>,
}

/// Acknowledgement of received/applied data.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Acknowledgement {
    pub acknowledged_bytes: u64,
    pub acknowledged_units: Vec<SyncUnitId>,
    pub receiver_state_hash: [u8; 32],
}

/// Cancellation reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CancelReason {
    UserRequested,
    BackpressureExceeded,
    LocalError(String),
    SwitchingBackend,
    AppBackgrounded,
}

/// Abort reason (fatal, stops exchange).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AbortReason {
    IncompatibleProtocol,
    VersionMismatch,
    Unauthorized,
    AuthorizationRevoked,
    FrameTooLarge,
    SessionTooLarge,
    MalformedFrame,
    MalformedMessage,
    ProtocolError(String),
    BackendUnavailable,
    StoreNotFound,
    ReplicaNotFound,
    DirectionNotGranted,
    SchemaIncompatible,
    InternalError(String),
}

/// Conflict report for concurrent edits.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConflictReport {
    pub store_id: Uuid,
    pub replica_id: Uuid,
    pub protocol_id: ProtocolId,
    pub conflicts: Vec<ConflictRecord>,
}

/// Individual conflict record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConflictRecord {
    pub unit_id: SyncUnitId,
    pub local_version: Vec<u8>,   // Protocol-specific version
    pub remote_version: Vec<u8>,  // Protocol-specific version
    pub local_payload: Vec<u8>,   // Opaque, encrypted for password manager
    pub remote_payload: Vec<u8>,  // Opaque
}

/// Top-level sync message enum.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SyncMessage {
    Hello(Hello),
    AttachRequest(AttachRequest),
    AttachResponse(AttachResponse),
    OwnerMessage {
        protocol_id: ProtocolId,
        payload: Vec<u8>,
    },
    OwnerMessageBatch {
        protocol_id: ProtocolId,
        payloads: Vec<Vec<u8>>,
    },
    Ack(Acknowledgement),
    Progress(ProgressReport),
    Cancel(CancelReason),
    Reconnect(ReconnectToken),
    Abort(AbortReason),
    Ping,
    Pong,
}

impl SyncMessage {
    pub fn encode(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}
