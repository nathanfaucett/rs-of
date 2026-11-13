#![cfg(feature = "sync")]

use std::{io, path::PathBuf, string::String, sync::Arc, vec::Vec};

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
use of_model::storage::StorageNamespace;
use storage_service::DatabaseRuntime;
use uuid::Uuid;

use crate::{DeviceClient, IdpClient, IdpIntrospectionError, ManagementClient, RouterState};

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
    pub fn encode(&self) -> Result<Vec<u8>, io::Error> {
        rmp_serde::to_vec(self).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn decode(data: &[u8]) -> Result<Self, io::Error> {
        rmp_serde::from_slice(data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
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
    CrcMismatch { expected: u32, actual: u32 },
    Truncated,
}

#[derive(Clone, Debug)]
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
        if frame.version != FRAMING_VERSION {
            return Err(FrameError::InvalidVersion {
                version: frame.version,
            });
        }

        let mut buf = Vec::with_capacity(4 + payload_len + 4);
        buf.push(frame.version);
        buf.push(frame.flags);
        buf.extend_from_slice(&(payload_len as u32).to_be_bytes());
        buf.extend_from_slice(&frame.payload);

        let crc = crc32fast::hash(&buf);
        buf.extend_from_slice(&crc.to_be_bytes());

        Ok(buf)
    }

    pub fn decode(buf: &[u8]) -> Result<Frame, FrameError> {
        if buf.len() < 8 {
            return Err(FrameError::Truncated);
        }

        let version = buf[0];
        if version != FRAMING_VERSION {
            return Err(FrameError::InvalidVersion { version });
        }

        let flags = buf[1];
        let payload_len = u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]) as usize;

        if buf.len() < 6 + payload_len + 4 {
            return Err(FrameError::Truncated);
        }

        let payload = &buf[6..6 + payload_len];
        let expected_crc = u32::from_be_bytes([
            buf[6 + payload_len],
            buf[7 + payload_len],
            buf[8 + payload_len],
            buf[9 + payload_len],
        ]);

        let mut crc_buf = buf[..6 + payload_len].to_vec();
        crc_buf.extend_from_slice(&[0, 0, 0, 0]);
        let actual_crc = crc32fast::hash(&crc_buf);

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

/// Sync authorization result.
///
/// The `target_device_id` identifies the peer device that has the same resource
/// selected for sync (via Management Service). For WebSocket sync, the server
/// is the authoritative replica; this field is logged for admin visibility.
/// Native peer-to-peer sync uses Iroh mesh separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncAuth {
    pub subject: String,
    pub application_id: String,
    pub store_id: Uuid,
    pub target_device_id: Uuid,
    pub direction: AttachDirection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncAuthError {
    Unauthorized,
    BackendUnavailable,
}

/// Authorizes a sync session by validating the token and checking Management selection.
///
/// Returns `SyncAuth` with the target device that has the same resource selected.
/// The server acts as the authoritative replica for WebSocket sync; the
/// `target_device_id` indicates which peer device also has sync enabled for
/// this resource (used for admin visibility and audit).
pub async fn authorize_sync(
    idp_client: &IdpClient,
    management_client: &ManagementClient,
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
    if claims.exp <= now || claims.nbf > now || claims.r#use != of_model::contract::TokenUse::Access {
        return Err(SyncAuthError::Unauthorized);
    }
    if claims.aud != "storage:sync" {
        return Err(SyncAuthError::Unauthorized);
    }

    // Parse device ID from endpoint_id (which is the device's Iroh public key)
    let device_id = Id::parse_str(endpoint_id).map_err(|_| SyncAuthError::Unauthorized)?;

    // Check if source device has selection for this resource
    let has_selection = management_client
        .has_selection(device_id, Id::from(store_identity.store_id))
        .await
        .map_err(|_| SyncAuthError::BackendUnavailable)?;

    if !has_selection {
        return Err(SyncAuthError::Unauthorized);
    }

    // Find the target device that has this resource selected
    let sync_target = management_client
        .get_sync_target(Id::from(store_identity.store_id), device_id)
        .await
        .map_err(|_| SyncAuthError::BackendUnavailable)?;

    let sync_target = sync_target.ok_or(SyncAuthError::BackendUnavailable)?;

    // Verify target device also has selection (get_sync_target only returns devices with selection)
    let target_has_selection = management_client
        .has_selection(sync_target.device_id, Id::from(store_identity.store_id))
        .await
        .map_err(|_| SyncAuthError::BackendUnavailable)?;

    if !target_has_selection {
        return Err(SyncAuthError::BackendUnavailable);
    }

    let app_id = introspection.application_id.clone();

    // Store target device ID for potential relay (future enhancement)
    tracing::debug!(
        "Sync authorized: source={}, target={}, resource={}",
        device_id,
        sync_target.device_id,
        store_identity.store_id
    );

    Ok(SyncAuth {
        subject: claims.sub.clone(),
        application_id: app_id,
        store_id: store_identity.store_id,
        target_device_id: sync_target.device_id,
        direction: AttachDirection::Bidirectional,
    })
}

fn unix_time_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs().min(i64::MAX as u64) as i64)
}

pub struct SyncState {
    pub router_state: RouterState,
    pub databases: Arc<DatabaseRuntime>,
    pub staging_dir: PathBuf,
}

pub fn sync_router(
    state: RouterState,
    databases: Arc<DatabaseRuntime>,
    staging_dir: PathBuf,
) -> Router {
    Router::new()
        .route("/storage/sync/ws", get(sync_ws_handler))
        .with_state(Arc::new(SyncState {
            router_state: state,
            databases,
            staging_dir,
        }))
}

async fn sync_ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<SyncState>>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

struct WsNamespace {
    subject: String,
    application_id: String,
}

impl StorageNamespace for WsNamespace {
    fn user_sub(&self) -> &str {
        &self.subject
    }

    fn application_id(&self) -> &str {
        &self.application_id
    }
}

struct SessionAuthorization<'a> {
    idp: &'a IdpClient,
    management: &'a ManagementClient,
    token: &'a str,
    identity: &'a StoreIdentity,
    endpoint_id: &'a str,
    expected: &'a Hello,
}

impl<'a> SessionAuthorization<'a> {
    async fn check(&self) -> Result<SyncAuth, SyncAuthError> {
        authorize_sync(
            self.idp,
            self.management,
            self.token,
            self.identity,
            self.endpoint_id,
        )
        .await
    }
}

struct WebSocketTransport<'a> {
    socket: &'a mut WebSocket,
    authorization: SyncAuth,
}

impl<'a> ofdb_sql::SyncTransport for WebSocketTransport<'a> {
    type Error = io::Error;

    async fn receive(&mut self) -> Result<Vec<u8>, Self::Error> {
        match self.socket.recv().await {
            Some(Ok(Message::Binary(data))) => Ok(data.to_vec()),
            Some(Ok(Message::Close(_))) => {
                Err(io::Error::new(io::ErrorKind::ConnectionAborted, "closed"))
            }
            Some(Err(e)) => Err(io::Error::new(io::ErrorKind::ConnectionAborted, e)),
            Some(Ok(_)) => self.receive().await,
            None => Err(io::Error::new(io::ErrorKind::ConnectionAborted, "closed")),
        }
    }

    async fn send(&mut self, payload: Vec<u8>) -> Result<(), Self::Error> {
        self.socket
            .send(Message::Binary(payload.into()))
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionAborted, e))
    }
}

async fn handle_socket(socket: WebSocket, state: Arc<SyncState>) {
    let mut socket = socket;
    let mut frame_decoder = FrameCodec;

    // Receive Hello
    let hello = match receive_message(&mut socket, &mut frame_decoder).await {
        Ok(SyncMessage::Hello(h)) => h,
        Ok(_) => {
            let _ = send_abort(&mut socket, AbortReason::MalformedMessage).await;
            return;
        }
        Err(_) => return,
    };

    // Send Hello response (echo)
    let hello_response = SyncMessage::Hello(Hello {
        protocol_version: hello.protocol_version,
        manifest: Vec::new(),
        store_identity: hello.store_identity.clone(),
        supported_protocols: vec![ProtocolId::OfdbSqlV7],
    });
    if send_message(&mut socket, &hello_response).await.is_err() {
        return;
    }

    // Receive AttachRequest
    let attach_req = match receive_message(&mut socket, &mut frame_decoder).await {
        Ok(SyncMessage::AttachRequest(req)) => req,
        Ok(_) => {
            let _ = send_abort(&mut socket, AbortReason::MalformedMessage).await;
            return;
        }
        Err(_) => return,
    };

    // Extract token from AttachRequest
    let token = String::from_utf8(attach_req.auth_token.0.clone()).unwrap_or_default();

    // Authorize the sync session
    let auth = match authorize_sync(
        &state.router_state.idp_client,
        &state.router_state.management_client,
        &token,
        &attach_req.store_identity,
        &state.router_state.endpoint_id,
    )
    .await
    {
        Ok(auth) => auth,
        Err(_) => {
            let _ = send_abort(&mut socket, AbortReason::Unauthorized).await;
            return;
        }
    };

    // Send AttachResponse
    let attach_resp = AttachResponse {
        accepted: true,
        backend_replica_id: Uuid::now_v7(),
        session_id: Uuid::now_v7(),
        granted_direction: AttachDirection::Bidirectional,
        max_frame_bytes: MAX_FRAME_PAYLOAD,
        max_session_bytes: MAX_SESSION_BYTES,
    };
    if send_message(&mut socket, &SyncMessage::AttachResponse(attach_resp))
        .await
        .is_err()
    {
        return;
    }

    // Run sync session
    let mut transport = WebSocketTransport {
        socket: &mut socket,
        authorization: auth,
    };
    let _ = run_sync_session(&mut transport, &state).await;
}

async fn receive_message(
    socket: &mut WebSocket,
    decoder: &mut FrameCodec,
) -> Result<SyncMessage, io::Error> {
    loop {
        match socket.recv().await {
            Some(Ok(Message::Binary(data))) => {
                let frame = FrameCodec::decode(&data)?;
                return SyncMessage::decode(&frame.payload);
            }
            Some(Ok(Message::Close(_))) => {
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "closed"));
            }
            Some(Err(e)) => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, e)),
            Some(Ok(_)) => continue,
            None => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "closed")),
        }
    }
}

async fn send_message(socket: &mut WebSocket, msg: &SyncMessage) -> Result<(), io::Error> {
    let encoded = msg.encode()?;
    let frame = Frame {
        version: FRAMING_VERSION,
        flags: 0,
        payload: encoded,
    };
    let frame_bytes = FrameCodec::encode(&frame)?;
    socket
        .send(Message::Binary(frame_bytes.into()))
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::ConnectionAborted, e))
}

async fn send_abort(socket: &mut WebSocket, reason: AbortReason) -> Result<(), io::Error> {
    send_message(socket, &SyncMessage::Abort(reason)).await
}

/// Runs a sync session between the client and the server's authoritative replica.
///
/// Architecture: The storage server holds the authoritative database replica.
/// Browser/server clients connect via WebSocket and sync *to* this server.
/// The `target_device_id` in `SyncAuth` is logged for admin visibility only
/// (the Management Service controls which devices have sync enabled for a resource).
/// Native apps use Iroh mesh for peer-to-peer sync separately.
async fn run_sync_session(
    transport: &mut WebSocketTransport<'_>,
    state: &Arc<SyncState>,
) -> Result<(), io::Error> {
    let db_runtime = &state.databases;
    let store_id = transport.authorization.store_id;
    // target_device_id is logged for admin visibility; server is authoritative replica
    let target_device_id = transport.authorization.target_device_id;

    tracing::debug!(
        "Running sync session for store {}, target device {} (authoritative replica)",
        store_id,
        target_device_id
    );

    // Get the database
    let database = db_runtime
        .get_database(store_id)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "database not found"))?;

    // Configure sync session
    let config = ofdb_sql::SessionConfig::default();

    // Run sync as responder (server side)
    database
        .synchronize(transport, &config, ofdb_sql::SyncRole::Responder)
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(())
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
        let encoded = FrameCodec::encode(&frame).unwrap();
        let decoded = FrameCodec::decode(&encoded).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn sync_message_roundtrip() {
        let msg = SyncMessage::Ping;
        let encoded = msg.encode().unwrap();
        let decoded = SyncMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[tokio::test]
    async fn sync_route_exists() {
        let _router = sync_router(
            RouterState::default(),
            Arc::new(DatabaseRuntime::default()),
            PathBuf::from("/tmp"),
        );
    }
}

#[cfg(test)]
mod authorization_tests {
    use super::*;
}
