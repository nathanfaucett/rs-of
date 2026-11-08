//! Frame encoding/decoding for sync protocol transport.

use std::vec::Vec;
use crc32fast::Hasher;
use thiserror::Error;

/// Maximum frame payload size (16 MiB).
pub const MAX_FRAME_PAYLOAD: u32 = 16 * 1024 * 1024;
/// Maximum session bytes (1 GiB).
pub const MAX_SESSION_BYTES: u64 = 1024 * 1024 * 1024;
/// Current framing version.
pub const FRAMING_VERSION: u8 = 1;

/// Frame flags.
pub mod flags {
    pub const CONTINUATION: u8 = 0x01;
    pub const COMPRESSED: u8 = 0x02;
    pub const PRIORITY: u8 = 0x04;
}

/// A sync protocol frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub version: u8,
    pub flags: u8,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(flags: u8, payload: Vec<u8>, continuation: bool) -> Self {
        let mut f = flags;
        if continuation {
            f |= flags::CONTINUATION;
        }
        Self {
            version: FRAMING_VERSION,
            flags: f,
            payload,
        }
    }

    pub fn is_continuation(&self) -> bool {
        (self.flags & flags::CONTINUATION) != 0
    }

    pub fn is_compressed(&self) -> bool {
        (self.flags & flags::COMPRESSED) != 0
    }

    pub fn is_priority(&self) -> bool {
        (self.flags & flags::PRIORITY) != 0
    }

    pub fn payload_len(&self) -> u32 {
        self.payload.len() as u32
    }
}

/// Frame encoding/decoding errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FrameError {
    #[error("frame too large: {size} > {max}")]
    FrameTooLarge { size: u32, max: u32 },
    #[error("invalid frame version: {version}")]
    InvalidVersion { version: u8 },
    #[error("crc32 mismatch: expected {expected}, got {actual}")]
    CrcMismatch { expected: u32, actual: u32 },
    #[error("truncated frame: need {need}, got {got}")]
    Truncated { need: usize, got: usize },
    #[error("empty frame")]
    EmptyFrame,
}

/// Frame codec for encoding/decoding frames.
pub struct FrameCodec;

impl FrameCodec {
    /// Encode a frame to bytes (version + flags + length + payload + crc32).
    pub fn encode(frame: &Frame) -> Result<Vec<u8>, FrameError> {
        let payload_len = frame.payload.len();
        if payload_len > MAX_FRAME_PAYLOAD as usize {
            return Err(FrameError::FrameTooLarge {
                size: payload_len as u32,
                max: MAX_FRAME_PAYLOAD,
            });
        }

        let mut buf = Vec::with_capacity(1 + 1 + 4 + payload_len + 4);
        buf.push(frame.version);
        buf.push(frame.flags);
        buf.extend_from_slice(&(payload_len as u32).to_le_bytes());
        buf.extend_from_slice(&frame.payload);

        // Compute CRC32C of payload only
        let mut hasher = Hasher::new();
        hasher.update(&frame.payload);
        let crc = hasher.finalize();
        buf.extend_from_slice(&crc.to_le_bytes());

        Ok(buf)
    }

    /// Decode a frame from bytes.
    pub fn decode(buf: &[u8]) -> Result<Frame, FrameError> {
        const HEADER_SIZE: usize = 1 + 1 + 4; // version + flags + length
        const CRC_SIZE: usize = 4;

        if buf.len() < HEADER_SIZE + CRC_SIZE {
            return Err(FrameError::Truncated {
                need: HEADER_SIZE + CRC_SIZE,
                got: buf.len(),
            });
        }

        let version = buf[0];
        if version != FRAMING_VERSION {
            return Err(FrameError::InvalidVersion { version });
        }

        let flags = buf[1];
        let length = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]) as usize;

        if length > MAX_FRAME_PAYLOAD as usize {
            return Err(FrameError::FrameTooLarge {
                size: length as u32,
                max: MAX_FRAME_PAYLOAD,
            });
        }

        let total_needed = HEADER_SIZE + length + CRC_SIZE;
        if buf.len() < total_needed {
            return Err(FrameError::Truncated {
                need: total_needed,
                got: buf.len(),
            });
        }

        let payload = &buf[HEADER_SIZE..HEADER_SIZE + length];
        let crc_bytes = &buf[HEADER_SIZE + length..HEADER_SIZE + length + CRC_SIZE];
        let expected_crc = u32::from_le_bytes([
            crc_bytes[0],
            crc_bytes[1],
            crc_bytes[2],
            crc_bytes[3],
        ]);

        // Verify CRC32C
        let mut hasher = Hasher::new();
        hasher.update(payload);
        let actual_crc = hasher.finalize();

        if actual_crc != expected_crc {
            return Err(FrameError::CrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }

        Ok(Frame {
            version,
            flags,
            payload: payload.to_vec(),
        })
    }
}

/// Streaming frame decoder for incremental reading.
pub struct FrameDecoder {
    buffer: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
        }
    }

    /// Feed data into the decoder, returning complete frames.
    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<Frame>, FrameError> {
        self.buffer.extend_from_slice(data);
        let mut frames = Vec::new();

        loop {
            if self.buffer.len() < 6 {
                break; // Need at least header
            }

            let length = u32::from_le_bytes([
                self.buffer[2],
                self.buffer[3],
                self.buffer[4],
                self.buffer[5],
            ]) as usize;

            let total_needed = 6 + length + 4;
            if self.buffer.len() < total_needed {
                break; // Incomplete frame
            }

            // Extract frame bytes
            let frame_bytes = &self.buffer[..total_needed];
            let frame = FrameCodec::decode(frame_bytes)?;
            frames.push(frame);

            // Remove processed bytes
            self.buffer.drain(..total_needed);
        }

        Ok(frames)
    }

    /// Get remaining buffered bytes (incomplete frame).
    pub fn remaining(&self) -> &[u8] {
        &self.buffer
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_encode_decode_roundtrip() {
        let frame = Frame::new(0, vec![1, 2, 3, 4], false);
        let encoded = FrameCodec::encode(&frame).unwrap();
        let decoded = FrameCodec::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn frame_with_continuation() {
        let frame = Frame::new(flags::CONTINUATION, vec![1, 2, 3], true);
        assert!(frame.is_continuation());
        let encoded = FrameCodec::encode(&frame).unwrap();
        let decoded = FrameCodec::decode(&encoded).unwrap();
        assert!(decoded.is_continuation());
    }

    #[test]
    fn frame_too_large_rejected() {
        let large_payload = vec![0u8; MAX_FRAME_PAYLOAD as usize + 1];
        let frame = Frame::new(0, large_payload, false);
        assert!(FrameCodec::encode(&frame).is_err());
    }

    #[test]
    fn streaming_decoder() {
        let mut decoder = FrameDecoder::new();
        let frame1 = Frame::new(0, vec![1, 2], false);
        let frame2 = Frame::new(0, vec![3, 4], false);
        let encoded1 = FrameCodec::encode(&frame1).unwrap();
        let encoded2 = FrameCodec::encode(&frame2).unwrap();

        // Feed first frame
        let frames = decoder.feed(&encoded1).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, vec![1, 2]);

        // Feed second frame
        let frames = decoder.feed(&encoded2).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, vec![3, 4]);

        // Feed partial
        let partial = &encoded1[..3];
        let frames = decoder.feed(partial).unwrap();
        assert_eq!(frames.len(), 0);
        assert_eq!(decoder.remaining().len(), 3);

        // Complete the frame
        let frames = decoder.feed(&encoded1[3..]).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, vec![1, 2]);
    }

    #[test]
    fn crc_mismatch_detected() {
        let frame = Frame::new(0, vec![1, 2, 3, 4], false);
        let mut encoded = FrameCodec::encode(&frame).unwrap();
        // Corrupt the payload
        encoded[6] ^= 0xFF;
        assert!(FrameCodec::decode(&encoded).is_err());
    }
}
