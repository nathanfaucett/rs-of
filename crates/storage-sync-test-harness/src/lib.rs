//! Test harness for app-owned data sync protocol.
//! Implements the protocol defined in sync-protocol.md for two-store proof.

#![forbid(unsafe_code)]

mod backend;
mod frames;
mod harness;
mod protocol;
mod transport;

pub use backend::TestBackend;
pub use frames::{Frame, FrameCodec, FrameError, MAX_FRAME_PAYLOAD, MAX_SESSION_BYTES};
pub use harness::{TwoStoreHarness, HarnessConfig, TestScenario, TestResult};
pub use protocol::{
    SyncMessage, Hello, AttachRequest, AttachResponse, AttachDirection,
    StoreIdentity, AuthToken, ProtocolId, SyncPhase, ProgressReport,
    Acknowledgement, CancelReason, ReconnectToken, ConflictRecord, ConflictReport,
    AbortReason, SyncSelection, SyncResult, SyncConfig, SyncUnitId,
};
pub use transport::{SyncTransport, TransportError};

#[cfg(feature = "native")]
pub use transport::native::WebSocketSyncTransport;

#[cfg(feature = "browser")]
pub use transport::browser::WebSocketSyncTransport;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_codec_roundtrip() {
        let frame = Frame::new(1, vec![1, 2, 3, 4], false);
        let encoded = FrameCodec::encode(&frame).expect("encode");
        let decoded = FrameCodec::decode(&encoded).expect("decode");
        assert_eq!(frame.version, decoded.version);
        assert_eq!(frame.flags, decoded.flags);
        assert_eq!(frame.payload, decoded.payload);
    }

    #[test]
    fn protocol_id_serialization() {
        let pid = ProtocolId::OfdbSqlV7;
        let encoded = postcard::to_allocvec(&pid).expect("encode");
        let decoded: ProtocolId = postcard::from_bytes(&encoded).expect("decode");
        assert_eq!(pid, decoded);
    }

    #[test]
    fn store_identity_serialization() {
        let identity = StoreIdentity {
            store_id: uuid::Uuid::new_v4(),
            replica_id: uuid::Uuid::new_v4(),
            creation_epoch: 1,
            schema_fingerprint: [0u8; 32],
        };
        let encoded = postcard::to_allocvec(&identity).expect("encode");
        let decoded: StoreIdentity = postcard::from_bytes(&encoded).expect("decode");
        assert_eq!(identity, decoded);
    }
}
