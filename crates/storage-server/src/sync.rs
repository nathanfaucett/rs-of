#![cfg(feature = "sync")]

use std::{string::String, vec::Vec};

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{DeviceClient, IdpClient, IdpIntrospectionError, RouterState};

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
    pub fn encode(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub version: u8,
    pub flags: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub enum FrameError {
    FrameTooLarge { size: u32, max: u32 },
    InvalidVersion { version: u8 },
    CrcMismatch { expected: u32, actual: u32 },
    Truncated,
}

pub struct FrameCodec;

impl FrameCodec {
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
        let mut hasher = Hasher::new();
        hasher.update(&frame.payload);
        buf.extend_from_slice(&hasher.finalize().to_le_bytes());
        Ok(buf)
    }

    pub fn decode(buf: &[u8]) -> Result<Frame, FrameError> {
        const HEADER: usize = 6;
        const CRC: usize = 4;
        if buf.len() < HEADER + CRC {
            return Err(FrameError::Truncated);
        }
        if buf[0] != FRAMING_VERSION {
            return Err(FrameError::InvalidVersion { version: buf[0] });
        }
        let length = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]) as usize;
        if length > MAX_FRAME_PAYLOAD as usize {
            return Err(FrameError::FrameTooLarge {
                size: length as u32,
                max: MAX_FRAME_PAYLOAD,
            });
        }
        if buf.len() < HEADER + length + CRC {
            return Err(FrameError::Truncated);
        }
        let payload = &buf[HEADER..HEADER + length];
        let expected = u32::from_le_bytes([
            buf[HEADER + length],
            buf[HEADER + length + 1],
            buf[HEADER + length + 2],
            buf[HEADER + length + 3],
        ]);
        let mut hasher = Hasher::new();
        hasher.update(payload);
        let actual = hasher.finalize();
        if actual != expected {
            return Err(FrameError::CrcMismatch { expected, actual });
        }
        Ok(Frame {
            version: buf[0],
            flags: buf[1],
            payload: payload.to_vec(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncAuth {
    pub subject: String,
    pub application_id: String,
    pub store_id: Uuid,
    pub direction: AttachDirection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncAuthError {
    Unauthorized,
    BackendUnavailable,
}

pub async fn authorize_sync(
    idp_client: &IdpClient,
    device_client: &DeviceClient,
    token: &str,
    store_identity: &StoreIdentity,
    endpoint_id: &str,
) -> Result<SyncAuth, SyncAuthError> {
    let introspection = idp_client
        .introspect(token)
        .await
        .map_err(|error| match error {
            IdpIntrospectionError::InvalidToken => SyncAuthError::Unauthorized,
            IdpIntrospectionError::ServiceUnavailable => SyncAuthError::BackendUnavailable,
        })?;

    let claims = &introspection.claims;
    let now = unix_time_seconds();
    if claims.exp <= now {
        return Err(SyncAuthError::Unauthorized);
    }
    if claims.aud != "storage:sync" {
        return Err(SyncAuthError::Unauthorized);
    }

    let selected = device_client
        .selected_resources(endpoint_id)
        .await
        .map_err(|_| SyncAuthError::BackendUnavailable)?;

    let store_id_str = store_identity.store_id.to_string();
    let app_id = introspection.application_id.clone();
    let authorized = selected
        .resources
        .iter()
        .any(|resource| resource.resource_id == store_id_str && resource.application_id == app_id);
    if !authorized {
        return Err(SyncAuthError::Unauthorized);
    }

    Ok(SyncAuth {
        subject: claims.sub.clone(),
        application_id: app_id,
        store_id: store_identity.store_id,
        direction: AttachDirection::Bidirectional,
    })
}

fn unix_time_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().min(i64::MAX as u64) as i64)
}

pub fn sync_router(state: RouterState) -> Router {
    Router::new()
        .route("/storage/sync/ws", get(sync_ws_handler))
        .with_state(state)
}

async fn sync_ws_handler(ws: WebSocketUpgrade, State(state): State<RouterState>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: RouterState) {
    while let Some(msg) = socket.recv().await {
        let Ok(msg) = msg else { break };
        match msg {
            Message::Binary(data) => {
                let frame = match FrameCodec::decode(&data) {
                    Ok(f) => f,
                    Err(_) => {
                        let _ = send_abort(&mut socket, AbortReason::MalformedMessage).await;
                        break;
                    }
                };
                let sync_msg = match SyncMessage::decode(&frame.payload) {
                    Ok(m) => m,
                    Err(_) => {
                        let _ = send_abort(&mut socket, AbortReason::MalformedMessage).await;
                        break;
                    }
                };
                match sync_msg {
                    SyncMessage::Ping => {
                        if send_message(&mut socket, &SyncMessage::Pong).await.is_err() {
                            break;
                        }
                    }
                    SyncMessage::AttachRequest(request) => {
                        let token = String::from_utf8_lossy(&request.auth_token.0).to_string();
                        let endpoint_id = "local";
                        let (idp, device) =
                            match (state.idp_client.as_ref(), state.device_client.as_ref()) {
                                (Some(idp), Some(device)) => (idp, device),
                                _ => {
                                    let _ =
                                        send_abort(&mut socket, AbortReason::BackendUnavailable)
                                            .await;
                                    break;
                                }
                            };
                        match authorize_sync(
                            idp,
                            device,
                            &token,
                            &request.store_identity,
                            endpoint_id,
                        )
                        .await
                        {
                            Ok(auth) => {
                                let response = AttachResponse {
                                    accepted: true,
                                    backend_replica_id: Uuid::now_v7(),
                                    session_id: Uuid::now_v7(),
                                    granted_direction: auth.direction,
                                    max_frame_bytes: MAX_FRAME_PAYLOAD,
                                    max_session_bytes: MAX_SESSION_BYTES,
                                };
                                if send_message(&mut socket, &SyncMessage::AttachResponse(response))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(SyncAuthError::Unauthorized) => {
                                let _ = send_abort(&mut socket, AbortReason::Unauthorized).await;
                                break;
                            }
                            Err(SyncAuthError::BackendUnavailable) => {
                                let _ =
                                    send_abort(&mut socket, AbortReason::BackendUnavailable).await;
                                break;
                            }
                        }
                    }
                    _ => {
                        if send_message(&mut socket, &sync_msg).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

async fn send_message(socket: &mut WebSocket, msg: &SyncMessage) -> Result<(), ()> {
    let payload = msg.encode().map_err(|_| ())?;
    let frame = Frame {
        version: FRAMING_VERSION,
        flags: 0,
        payload,
    };
    let bytes = FrameCodec::encode(&frame).map_err(|_| ())?;
    socket
        .send(Message::Binary(bytes.into()))
        .await
        .map_err(|_| ())
}

async fn send_abort(socket: &mut WebSocket, reason: AbortReason) -> Result<(), ()> {
    send_message(socket, &SyncMessage::Abort(reason)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip() {
        let frame = Frame {
            version: FRAMING_VERSION,
            flags: 0,
            payload: vec![1, 2, 3, 4],
        };
        let encoded = FrameCodec::encode(&frame).expect("encode");
        let decoded = FrameCodec::decode(&encoded).expect("decode");
        assert_eq!(frame, decoded);
    }

    #[test]
    fn sync_message_roundtrip() {
        let msg = SyncMessage::Ping;
        let bytes = msg.encode().expect("encode");
        let decoded = SyncMessage::decode(&bytes).expect("decode");
        assert_eq!(msg, decoded);
    }

    #[tokio::test]
    async fn sync_route_exists() {
        use axum::{body::Body, http::Request};
        use tower::ServiceExt;

        let router = sync_router(RouterState::new("http://localhost"));
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/storage/sync/ws")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_ne!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }
}
