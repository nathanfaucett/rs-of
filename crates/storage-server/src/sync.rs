#![cfg(feature = "sync")]

use std::{io, path::PathBuf, sync::Arc};

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use of_model::{idp::model::Id, storage::StorageNamespace};
use ofdb_sql::history::{SessionConfig, SyncRole, SyncTransport};
use serde::{Deserialize, Serialize};
use storage_service::DatabaseRuntime;
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

pub struct FrameCodec;

impl FrameCodec {
    pub fn encode(frame: &Frame) -> Result<Vec<u8>, FrameError> {
        if frame.version != FRAMING_VERSION {
            return Err(FrameError::InvalidVersion {
                version: frame.version,
            });
        }
        if frame.flags != 0 {
            return Err(FrameError::InvalidFlags);
        }
        let length = frame.payload.len();
        if length > MAX_FRAME_PAYLOAD as usize {
            return Err(FrameError::FrameTooLarge {
                size: length.min(u32::MAX as usize) as u32,
                max: MAX_FRAME_PAYLOAD,
            });
        }
        let mut bytes = Vec::with_capacity(length + 10);
        bytes.extend_from_slice(&[frame.version, frame.flags]);
        bytes.extend_from_slice(&(length as u32).to_le_bytes());
        bytes.extend_from_slice(&frame.payload);
        bytes.extend_from_slice(&crc32fast::hash(&frame.payload).to_le_bytes());
        Ok(bytes)
    }

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
        let length = u32::from_le_bytes(bytes[2..6].try_into().expect("four length bytes"));
        if length > MAX_FRAME_PAYLOAD {
            return Err(FrameError::FrameTooLarge {
                size: length,
                max: MAX_FRAME_PAYLOAD,
            });
        }
        let end = 6 + length as usize;
        if bytes.len() < end + 4 {
            return Err(FrameError::Truncated);
        }
        if bytes.len() != end + 4 {
            return Err(FrameError::TrailingBytes);
        }
        let expected = u32::from_le_bytes(bytes[end..].try_into().expect("four CRC bytes"));
        let actual = crc32fast::hash(&bytes[6..end]);
        if actual != expected {
            return Err(FrameError::CrcMismatch { expected, actual });
        }
        Ok(Frame {
            version: bytes[0],
            flags: bytes[1],
            payload: bytes[6..end].to_vec(),
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
    idp: &IdpClient,
    device: &DeviceClient,
    token: &str,
    identity: &StoreIdentity,
    endpoint_id: &str,
) -> Result<SyncAuth, SyncAuthError> {
    let introspection = idp.introspect(token).await.map_err(|error| match error {
        IdpIntrospectionError::InvalidToken => SyncAuthError::Unauthorized,
        IdpIntrospectionError::ServiceUnavailable => SyncAuthError::BackendUnavailable,
    })?;
    let claims = &introspection.claims;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs().min(i64::MAX as u64) as i64);
    if claims.exp <= now
        || claims.nbf > now
        || claims.r#use != of_model::contract::TokenUse::Access
        || claims.aud != "storage:sync"
    {
        return Err(SyncAuthError::Unauthorized);
    }
    let selected = device
        .selected_resources(endpoint_id)
        .await
        .map_err(|_| SyncAuthError::BackendUnavailable)?;
    if selected.endpoint_id != endpoint_id
        || selected.owner_subject != claims.sub
        || !selected.admin_allowed
        || selected.pending_cleanup.is_some()
        || !selected.resources.iter().any(|resource| {
            resource.owner_subject == claims.sub
                && resource.application_id == introspection.application_id
                && resource.resource_id == identity.store_id.to_string()
                && resource.kind == "database"
        })
    {
        return Err(SyncAuthError::Unauthorized);
    }
    Ok(SyncAuth {
        subject: claims.sub.clone(),
        application_id: introspection.application_id,
        store_id: identity.store_id,
        direction: AttachDirection::Bidirectional,
    })
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
    ws.max_message_size(MAX_FRAME_PAYLOAD as usize + 10)
        .max_frame_size(MAX_FRAME_PAYLOAD as usize + 10)
        .on_upgrade(move |socket| handle_socket(socket, state))
}

struct WsNamespace {
    subject: String,
    application_id: Id,
}

impl StorageNamespace for WsNamespace {
    fn user_sub(&self) -> &str {
        &self.subject
    }
    fn application_id(&self) -> Id {
        self.application_id
    }
}

struct SessionAuthorization<'a> {
    idp: &'a IdpClient,
    device: &'a DeviceClient,
    token: &'a str,
    identity: &'a StoreIdentity,
    endpoint_id: &'a str,
    expected: &'a SyncAuth,
}

impl SessionAuthorization<'_> {
    async fn check(&self) -> io::Result<()> {
        match authorize_sync(
            self.idp,
            self.device,
            self.token,
            self.identity,
            self.endpoint_id,
        )
        .await
        {
            Ok(current) if &current == self.expected => Ok(()),
            _ => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "sync authorization unavailable or revoked",
            )),
        }
    }
}

struct WireSocket {
    socket: WebSocket,
    bytes: u64,
}

impl WireSocket {
    fn account(&mut self, bytes: usize) -> io::Result<()> {
        self.bytes = self.bytes.saturating_add(bytes as u64);
        if self.bytes > MAX_SESSION_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "sync session byte limit exceeded",
            ));
        }
        Ok(())
    }

    async fn receive(&mut self) -> io::Result<SyncMessage> {
        loop {
            match self.socket.recv().await {
                Some(Ok(Message::Binary(bytes))) => {
                    self.account(bytes.len())?;
                    let frame = FrameCodec::decode(&bytes).map_err(|error| {
                        io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}"))
                    })?;
                    return SyncMessage::decode(&frame.payload).map_err(io::Error::other);
                }
                Some(Ok(Message::Ping(bytes) | Message::Pong(bytes))) => {
                    self.account(bytes.len())?
                }
                Some(Ok(Message::Close(_))) | None => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "sync socket closed",
                    ));
                }
                Some(Ok(Message::Text(_))) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "binary sync frame required",
                    ));
                }
                Some(Err(error)) => return Err(io::Error::other(error)),
            }
        }
    }

    async fn send(&mut self, message: SyncMessage) -> io::Result<()> {
        let payload = message.encode().map_err(io::Error::other)?;
        let bytes = FrameCodec::encode(&Frame {
            version: FRAMING_VERSION,
            flags: 0,
            payload,
        })
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}")))?;
        self.account(bytes.len())?;
        self.socket
            .send(Message::Binary(bytes.into()))
            .await
            .map_err(io::Error::other)
    }
}

struct WebSocketTransport<'a> {
    wire: &'a mut WireSocket,
    authorization: &'a SessionAuthorization<'a>,
}

impl SyncTransport for WebSocketTransport<'_> {
    type Error = io::Error;

    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        loop {
            self.authorization.check().await?;
            let message = self.wire.receive().await?;
            self.authorization.check().await?;
            match message {
                SyncMessage::OwnerMessage {
                    protocol_id: ProtocolId::OfdbSqlV7,
                    payload,
                } => return Ok(payload),
                SyncMessage::Ping => self.wire.send(SyncMessage::Pong).await?,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unexpected sync owner message",
                    ));
                }
            }
        }
    }

    async fn send(&mut self, payload: Vec<u8>) -> io::Result<()> {
        self.authorization.check().await?;
        self.wire
            .send(SyncMessage::OwnerMessage {
                protocol_id: ProtocolId::OfdbSqlV7,
                payload,
            })
            .await
    }
}

async fn handle_socket(socket: WebSocket, state: Arc<SyncState>) {
    let mut wire = WireSocket { socket, bytes: 0 };
    if let Err(reason) = attach_and_sync(&mut wire, &state).await {
        let _ = wire.send(SyncMessage::Abort(reason)).await;
    }
}

async fn attach_and_sync(wire: &mut WireSocket, state: &SyncState) -> Result<(), AbortReason> {
    let hello = match wire.receive().await {
        Ok(SyncMessage::Hello(hello))
            if hello.protocol_version == 1
                && hello.supported_protocols.contains(&ProtocolId::OfdbSqlV7) =>
        {
            hello
        }
        _ => return Err(AbortReason::MalformedMessage),
    };
    wire.send(SyncMessage::Hello(Hello {
        protocol_version: 1,
        manifest: Vec::new(),
        store_identity: hello.store_identity.clone(),
        supported_protocols: vec![ProtocolId::OfdbSqlV7],
    }))
    .await
    .map_err(|_| AbortReason::MalformedMessage)?;
    let request = match wire.receive().await {
        Ok(SyncMessage::AttachRequest(request))
            if request.store_identity == hello.store_identity
                && request.direction == AttachDirection::Bidirectional =>
        {
            request
        }
        _ => return Err(AbortReason::MalformedMessage),
    };
    let token = String::from_utf8(request.auth_token.0).map_err(|_| AbortReason::Unauthorized)?;
    let (Some(idp), Some(device)) = (
        &state.router_state.idp_client,
        &state.router_state.device_client,
    ) else {
        return Err(AbortReason::BackendUnavailable);
    };
    let auth = authorize_sync(
        idp,
        device,
        &token,
        &request.store_identity,
        &state.router_state.endpoint_id,
    )
    .await
    .map_err(|error| match error {
        SyncAuthError::Unauthorized => AbortReason::Unauthorized,
        SyncAuthError::BackendUnavailable => AbortReason::BackendUnavailable,
    })?;
    let authorization = SessionAuthorization {
        idp,
        device,
        token: &token,
        identity: &request.store_identity,
        endpoint_id: &state.router_state.endpoint_id,
        expected: &auth,
    };
    let namespace = WsNamespace {
        subject: auth.subject.clone(),
        application_id: auth
            .application_id
            .parse()
            .map_err(|_| AbortReason::Unauthorized)?,
    };
    let database = state
        .databases
        .open_selected(&namespace, auth.store_id.into())
        .map_err(|_| AbortReason::BackendUnavailable)?
        .ok_or(AbortReason::BackendUnavailable)?;
    authorization
        .check()
        .await
        .map_err(|_| AbortReason::Unauthorized)?;
    wire.send(SyncMessage::AttachResponse(AttachResponse {
        accepted: true,
        backend_replica_id: Uuid::now_v7(),
        session_id: Uuid::now_v7(),
        granted_direction: auth.direction,
        max_frame_bytes: MAX_FRAME_PAYLOAD,
        max_session_bytes: MAX_SESSION_BYTES,
    }))
    .await
    .map_err(|_| AbortReason::BackendUnavailable)?;
    database
        .exchange_resource_history(
            &mut WebSocketTransport {
                wire,
                authorization: &authorization,
            },
            &SessionConfig::default(),
            SyncRole::Responder,
            &state.staging_dir,
            || async {
                authorization
                    .check()
                    .await
                    .map_err(|error| error.to_string())
            },
        )
        .await
        .map_err(|error| match error {
            ofdb_sql::history::SyncError::PreApply(_) => AbortReason::Unauthorized,
            ofdb_sql::history::SyncError::Transport(error)
                if error.kind() == io::ErrorKind::PermissionDenied =>
            {
                AbortReason::Unauthorized
            }
            error => {
                log::debug!("Storage WebSocket sync failed: {error}");
                AbortReason::MalformedMessage
            }
        })?;
    Ok(())
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "sync_authorization_tests.rs"]
mod authorization_tests;
