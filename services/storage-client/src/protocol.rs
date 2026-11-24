use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_FRAME_PAYLOAD: u32 = 16 * 1024 * 1024;
pub const MAX_SESSION_BYTES: u64 = 1024 * 1024 * 1024;
pub const FRAMING_VERSION: u8 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProtocolId {
    OfdbSqlV7,
    Other(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoreIdentity {
    pub store_id: Uuid,
    pub replica_id: Uuid,
    pub creation_epoch: u64,
    pub schema_fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthToken(pub Vec<u8>);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum AttachDirection {
    Push = 0,
    Pull = 1,
    Bidirectional = 2,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol_version: u16,
    pub manifest: Vec<u8>,
    pub store_identity: StoreIdentity,
    pub supported_protocols: Vec<ProtocolId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttachRequest {
    pub store_identity: StoreIdentity,
    pub auth_token: AuthToken,
    pub direction: AttachDirection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttachResponse {
    pub accepted: bool,
    pub backend_replica_id: Uuid,
    pub session_id: Uuid,
    pub granted_direction: AttachDirection,
    pub max_frame_bytes: u32,
    pub max_session_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AbortReason {
    Unauthorized,
    BackendUnavailable,
    MalformedMessage,
    InternalError(String),
}

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
    Abort(AbortReason),
    Ping,
    Pong,
}

impl SyncMessage {
    /// Encodes one protocol message.
    ///
    /// # Errors
    /// Returns an error if Postcard serialization fails.
    pub fn encode(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    /// Decodes one complete protocol message.
    ///
    /// # Errors
    /// Returns an error if Postcard decoding fails or trailing bytes remain.
    pub fn decode(bytes: &[u8]) -> Result<Self, postcard::Error> {
        let (message, remaining) = postcard::take_from_bytes(bytes)?;
        if !remaining.is_empty() {
            return Err(postcard::Error::DeserializeBadEncoding);
        }
        Ok(message)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub version: u8,
    pub flags: u8,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameError {
    FrameTooLarge { size: u32, max: u32 },
    InvalidVersion { version: u8 },
    InvalidFlags,
    CrcMismatch { expected: u32, actual: u32 },
    Truncated,
    TrailingBytes,
}

#[derive(Debug)]
pub struct FrameCodec;

impl FrameCodec {
    /// Encodes one checked wire frame.
    ///
    /// # Errors
    /// Returns an error if the version or flags are invalid, or the payload exceeds the wire limit.
    pub fn encode(frame: &Frame) -> Result<Vec<u8>, FrameError> {
        if frame.version != FRAMING_VERSION {
            return Err(FrameError::InvalidVersion {
                version: frame.version,
            });
        }
        if frame.flags != 0 {
            return Err(FrameError::InvalidFlags);
        }
        let payload_len = frame.payload.len();
        let length = u32::try_from(payload_len).unwrap_or(u32::MAX);
        if length > MAX_FRAME_PAYLOAD {
            return Err(FrameError::FrameTooLarge {
                size: length,
                max: MAX_FRAME_PAYLOAD,
            });
        }
        let mut bytes = Vec::with_capacity(payload_len + 10);
        bytes.extend_from_slice(&[frame.version, frame.flags]);
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(&frame.payload);
        bytes.extend_from_slice(&crc32fast::hash(&frame.payload).to_le_bytes());
        Ok(bytes)
    }

    /// Decodes and validates one complete wire frame.
    ///
    /// # Errors
    /// Returns an error for invalid framing, an excessive payload, truncation, trailing bytes, or
    /// a CRC mismatch.
    pub fn decode(bytes: &[u8]) -> Result<Frame, FrameError> {
        if bytes.len() < 10 {
            return Err(FrameError::Truncated);
        }
        if bytes[0] != FRAMING_VERSION {
            return Err(FrameError::InvalidVersion { version: bytes[0] });
        }
        if bytes[1] != 0 {
            return Err(FrameError::InvalidFlags);
        }
        let length = u32::from_le_bytes(
            bytes
                .get(2..6)
                .ok_or(FrameError::Truncated)?
                .try_into()
                .map_err(|_error| FrameError::Truncated)?,
        );
        if length > MAX_FRAME_PAYLOAD {
            return Err(FrameError::FrameTooLarge {
                size: length,
                max: MAX_FRAME_PAYLOAD,
            });
        }
        let end = 6 + usize::try_from(length).map_err(|_error| FrameError::FrameTooLarge {
            size: length,
            max: MAX_FRAME_PAYLOAD,
        })?;
        if bytes.len() < end + 4 {
            return Err(FrameError::Truncated);
        }
        if bytes.len() != end + 4 {
            return Err(FrameError::TrailingBytes);
        }
        let expected = u32::from_le_bytes(
            bytes
                .get(end..)
                .ok_or(FrameError::Truncated)?
                .try_into()
                .map_err(|_error| FrameError::Truncated)?,
        );
        let payload = bytes.get(6..end).ok_or(FrameError::Truncated)?;
        let actual = crc32fast::hash(payload);
        if actual != expected {
            return Err(FrameError::CrcMismatch { expected, actual });
        }
        Ok(Frame {
            version: bytes[0],
            flags: bytes[1],
            payload: payload.to_vec(),
        })
    }
}
