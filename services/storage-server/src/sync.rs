#![cfg(feature = "sync")]

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::get,
};
use of_model::{id::Id, storage::namespace::StorageNamespace};
use ofdb_sql::{
    Database, SessionConfig, SyncRole, SyncTransport,
    history::{HistoryPublication, HistoryRevision, SyncError},
};
use serde::{Deserialize, Serialize};
use storage_service::database_runtime::DatabaseRuntime;
use tokio::time::timeout;
use uuid::Uuid;

use crate::{
    device_client::DeviceClient,
    idp_client::IdpClient,
    router::{
        state::RouterState,
        storage_authorization::{
            StorageAuthorization, StorageAuthorizationError, authorize_inspected_token,
            authorize_storage_token, inspect_storage_token_read_only,
        },
    },
};

pub const MAX_FRAME_PAYLOAD: u32 = 16 * 1024 * 1024;
pub const MAX_SESSION_BYTES: u64 = 1024 * 1024 * 1024;
pub const FRAMING_VERSION: u8 = 1;
const PUBLISH_AUTH_TIMEOUT: Duration = Duration::from_secs(30);
const COMPARE_SESSION_TIMEOUT: Duration = Duration::from_secs(120);

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
    DestinationHasHistory,
    DestinationChanged,
    PublicationAlreadyUsed,
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
    FirstUpload(AttachRequest),
    FirstUploadPublished,
    RevisionedPull(AttachRequest),
    SnapshotRevision(HistoryRevision),
    ComparePublish(AttachRequest, HistoryPublication),
    ComparePublished(HistoryPublication),
    CheckPublication(AttachRequest, HistoryPublication),
    PublicationStatus(bool),
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
    pub expires_at: i64,
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
    authorize_device_sync(
        idp,
        device,
        token,
        identity,
        endpoint_id,
        AttachDirection::Bidirectional,
        AuthorityCheck::Reconcile,
    )
    .await
}

fn unix_time_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs().min(i64::MAX as u64) as i64)
}

#[derive(Clone, Copy)]
enum AuthorityCheck {
    Reconcile,
    Snapshot { renew: bool },
}

async fn authorize_device_sync(
    idp: &IdpClient,
    device: &DeviceClient,
    token: &str,
    identity: &StoreIdentity,
    endpoint_id: &str,
    direction: AttachDirection,
    authority: AuthorityCheck,
) -> Result<SyncAuth, SyncAuthError> {
    let introspection = match authority {
        AuthorityCheck::Reconcile => idp.introspect(token).await,
        AuthorityCheck::Snapshot { renew: true } => idp.inspect_identity(token).await,
        AuthorityCheck::Snapshot { renew: false } => idp.inspect_identity_read_only(token).await,
    };
    if introspection
        .as_ref()
        .is_ok_and(|response| response.claims.nbf > unix_time_seconds())
    {
        return Err(SyncAuthError::Unauthorized);
    }
    let auth = authorize_direction(
        authorize_inspected_token(introspection),
        identity,
        direction,
    )?;
    let selected = match authority {
        AuthorityCheck::Reconcile => device.selected_resources(endpoint_id).await,
        AuthorityCheck::Snapshot { renew } => device.selection_snapshot(endpoint_id, renew).await,
    }
    .map_err(|_error| SyncAuthError::BackendUnavailable)?;
    if auth.expires_at <= unix_time_seconds()
        || selected.endpoint_id != endpoint_id
        || selected.owner_subject != auth.subject
        || !selected.admin_allowed
        || selected.pending_cleanup.is_some()
        || !selected.resources.iter().any(|resource| {
            resource.owner_subject == auth.subject
                && resource.application_id == auth.application_id
                && resource.resource_id == identity.store_id.to_string()
                && resource.kind == "database"
        })
    {
        return Err(SyncAuthError::Unauthorized);
    }
    Ok(auth)
}

async fn authorize_session(
    idp: &IdpClient,
    device: Option<&DeviceClient>,
    token: &str,
    identity: &StoreIdentity,
    endpoint_id: &str,
    direction: AttachDirection,
    authority: AuthorityCheck,
) -> Result<SyncAuth, SyncAuthError> {
    if let Some(device) = device {
        return authorize_device_sync(
            idp,
            device,
            token,
            identity,
            endpoint_id,
            direction,
            authority,
        )
        .await;
    }
    let authorization = match authority {
        AuthorityCheck::Reconcile => authorize_storage_token(idp, token).await,
        AuthorityCheck::Snapshot { renew } if !idp.has_local_device_policy() => {
            inspect_storage_token_read_only(idp, token, renew).await
        }
        AuthorityCheck::Snapshot { .. } => Err(StorageAuthorizationError::InvalidToken),
    };
    authorize_direction(authorization, identity, direction)
}

fn authorize_direction(
    authorization: Result<StorageAuthorization, StorageAuthorizationError>,
    identity: &StoreIdentity,
    direction: AttachDirection,
) -> Result<SyncAuth, SyncAuthError> {
    let authorization = authorization.map_err(|error| match error {
        StorageAuthorizationError::InvalidToken => SyncAuthError::Unauthorized,
        StorageAuthorizationError::ServiceUnavailable => SyncAuthError::BackendUnavailable,
    })?;
    if authorization.resource != "storage:sync"
        || !authorization.can_read
        || match direction {
            AttachDirection::Pull => false,
            AttachDirection::Bidirectional => !authorization.can_write,
            AttachDirection::Push => true,
        }
    {
        return Err(SyncAuthError::Unauthorized);
    }
    Ok(SyncAuth {
        subject: authorization.subject,
        application_id: authorization.application_id,
        store_id: identity.store_id,
        direction,
        expires_at: authorization.expires_at,
    })
}

pub struct SyncState {
    pub router_state: RouterState,
    pub databases: Arc<DatabaseRuntime>,
    pub staging_dir: PathBuf,
    local: bool,
}

pub fn sync_router(
    state: RouterState,
    databases: Arc<DatabaseRuntime>,
    staging_dir: PathBuf,
) -> Router {
    build_sync_router(state, databases, staging_dir, false)
}

pub fn local_sync_router(
    state: RouterState,
    databases: Arc<DatabaseRuntime>,
    staging_dir: PathBuf,
) -> Router {
    build_sync_router(state, databases, staging_dir, true)
}

fn build_sync_router(
    state: RouterState,
    databases: Arc<DatabaseRuntime>,
    staging_dir: PathBuf,
    local: bool,
) -> Router {
    Router::new()
        .route("/storage/sync/ws", get(sync_ws_handler))
        .with_state(Arc::new(SyncState {
            router_state: state,
            databases,
            staging_dir,
            local,
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
    device: Option<&'a DeviceClient>,
    token: &'a str,
    identity: &'a StoreIdentity,
    endpoint_id: &'a str,
    expected: &'a SyncAuth,
}

impl SessionAuthorization<'_> {
    async fn check(&self) -> io::Result<()> {
        self.check_authority(if self.expected.direction == AttachDirection::Pull {
            AuthorityCheck::Snapshot { renew: false }
        } else {
            AuthorityCheck::Reconcile
        })
        .await
    }

    async fn check_publish(&self) -> io::Result<()> {
        self.check_authority(AuthorityCheck::Snapshot { renew: false })
            .await
    }

    async fn check_authority(&self, authority: AuthorityCheck) -> io::Result<()> {
        self.require_current(
            authorize_session(
                self.idp,
                self.device,
                self.token,
                self.identity,
                self.endpoint_id,
                self.expected.direction,
                authority,
            )
            .await,
        )
    }

    fn require_current(&self, result: Result<SyncAuth, SyncAuthError>) -> io::Result<()> {
        match result {
            Ok(current)
                if &current == self.expected && current.expires_at > unix_time_seconds() =>
            {
                Ok(())
            }
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
    compare: bool,
}

impl WebSocketTransport<'_> {
    async fn check(&mut self) -> io::Result<()> {
        if self.compare {
            self.authorization.check_publish().await
        } else {
            self.authorization.check().await
        }
    }
}

impl SyncTransport for WebSocketTransport<'_> {
    type Error = io::Error;

    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        loop {
            self.check().await?;
            let message = self.wire.receive().await?;
            self.check().await?;
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
        self.check().await?;
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

#[derive(Clone, Copy)]
enum Intent {
    Exchange,
    FirstUpload,
    RevisionedPull,
    Compare(HistoryPublication),
    Check(HistoryPublication),
}

impl Intent {
    fn direction_is_valid(self, direction: AttachDirection) -> bool {
        match self {
            Self::Exchange => direction != AttachDirection::Push,
            Self::RevisionedPull => direction == AttachDirection::Pull,
            _ => direction == AttachDirection::Bidirectional,
        }
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
    let (request, intent) = match wire.receive().await {
        Ok(SyncMessage::AttachRequest(request)) => (request, Intent::Exchange),
        Ok(SyncMessage::FirstUpload(request)) => (request, Intent::FirstUpload),
        Ok(SyncMessage::RevisionedPull(request)) => (request, Intent::RevisionedPull),
        Ok(SyncMessage::ComparePublish(request, publication)) => {
            (request, Intent::Compare(publication))
        }
        Ok(SyncMessage::CheckPublication(request, publication)) => {
            (request, Intent::Check(publication))
        }
        _ => return Err(AbortReason::MalformedMessage),
    };
    if request.store_identity != hello.store_identity
        || !intent.direction_is_valid(request.direction)
    {
        return Err(AbortReason::MalformedMessage);
    }
    let token = String::from_utf8(request.auth_token.0).map_err(|_| AbortReason::Unauthorized)?;
    let idp = state
        .router_state
        .idp_client
        .as_ref()
        .ok_or(AbortReason::BackendUnavailable)?;
    let device = if state.local {
        None
    } else {
        Some(
            state
                .router_state
                .device_client
                .as_ref()
                .ok_or(AbortReason::BackendUnavailable)?,
        )
    };
    let auth = authorize_session(
        idp,
        device,
        &token,
        &request.store_identity,
        &state.router_state.endpoint_id,
        request.direction,
        if request.direction == AttachDirection::Pull
            || matches!(intent, Intent::Compare(_) | Intent::Check(_))
        {
            AuthorityCheck::Snapshot { renew: true }
        } else {
            AuthorityCheck::Reconcile
        },
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
    let database = match intent {
        Intent::Check(_) => state
            .databases
            .open_existing_for_receipt(&namespace, auth.store_id.into()),
        Intent::RevisionedPull | Intent::Compare(_) => state
            .databases
            .open_existing_for_pull(&namespace, auth.store_id.into()),
        Intent::Exchange if auth.direction == AttachDirection::Pull => state
            .databases
            .open_existing_for_pull(&namespace, auth.store_id.into()),
        _ => state
            .databases
            .open_selected(&namespace, auth.store_id.into()),
    }
    .map_err(|_| AbortReason::BackendUnavailable)?
    .ok_or(AbortReason::BackendUnavailable)?;
    let eligible = if matches!(intent, Intent::FirstUpload) {
        database
            .first_upload_is_eligible()
            .await
            .map_err(|_error| AbortReason::BackendUnavailable)?
    } else {
        true
    };
    if matches!(intent, Intent::Compare(_) | Intent::Check(_)) {
        authorization.check_publish().await
    } else {
        authorization.check().await
    }
    .map_err(|_error| AbortReason::Unauthorized)?;
    if !eligible {
        return Err(AbortReason::DestinationHasHistory);
    }
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
    if let Intent::Check(publication) = intent {
        let status = database.check_history_publication(publication).await;
        authorization
            .check_publish()
            .await
            .map_err(|_| AbortReason::Unauthorized)?;
        let published = status.map_err(|error| match error {
            SyncError::PublicationAlreadyUsed => AbortReason::PublicationAlreadyUsed,
            _ => AbortReason::BackendUnavailable,
        })?;
        return wire
            .send(SyncMessage::PublicationStatus(published))
            .await
            .map_err(|_| AbortReason::BackendUnavailable);
    }
    let exchange = exchange_attached_history(
        WebSocketTransport {
            wire,
            authorization: &authorization,
            compare: matches!(intent, Intent::Compare(_)),
        },
        &database,
        &state.staging_dir,
        intent,
    );
    if matches!(intent, Intent::Compare(_) | Intent::RevisionedPull) {
        timeout(COMPARE_SESSION_TIMEOUT, exchange)
            .await
            .map_err(|_| AbortReason::BackendUnavailable)?
    } else {
        exchange.await
    }
}

async fn exchange_attached_history(
    mut transport: WebSocketTransport<'_>,
    database: &Database,
    staging_dir: &Path,
    intent: Intent,
) -> Result<(), AbortReason> {
    let authorization = transport.authorization;
    let check = || async {
        if matches!(intent, Intent::Compare(_)) {
            authorization.check_publish().await
        } else {
            authorization.check().await
        }
        .map_err(|error| error.to_string())
    };
    let config = SessionConfig::default();
    let check_publish = || async {
        timeout(PUBLISH_AUTH_TIMEOUT, authorization.check_publish())
            .await
            .map_err(|_elapsed| "publish authorization timed out".to_owned())?
            .map_err(|error| error.to_string())
    };
    if let Intent::Compare(publication) = intent {
        database
            .receive_history_publication(
                &mut transport,
                &config,
                staging_dir,
                publication,
                check,
                check_publish,
            )
            .await
            .map_err(exchange_abort)?;
        transport
            .wire
            .send(SyncMessage::ComparePublished(publication))
            .await
            .map_err(|_| AbortReason::BackendUnavailable)?;
    } else if matches!(intent, Intent::FirstUpload) {
        database
            .receive_first_upload(&mut transport, &config, staging_dir, check, check_publish)
            .await
            .map_err(exchange_abort)?;
        transport
            .wire
            .send(SyncMessage::FirstUploadPublished)
            .await
            .map_err(|_| AbortReason::BackendUnavailable)?;
    } else if authorization.expected.direction == AttachDirection::Pull {
        let (_, revision) = database
            .serve_revisioned_resource_history(&mut transport, &config, staging_dir)
            .await
            .map_err(exchange_abort)?;
        if matches!(intent, Intent::RevisionedPull) {
            authorization
                .check_publish()
                .await
                .map_err(|_| AbortReason::Unauthorized)?;
            transport
                .wire
                .send(SyncMessage::SnapshotRevision(revision))
                .await
                .map_err(|_| AbortReason::BackendUnavailable)?;
        }
    } else {
        database
            .exchange_resource_history(
                &mut transport,
                &config,
                SyncRole::Responder,
                staging_dir,
                check,
            )
            .await
            .map_err(exchange_abort)?;
    }
    Ok(())
}

fn exchange_abort(error: SyncError<io::Error>) -> AbortReason {
    match error {
        SyncError::DestinationHasHistory => AbortReason::DestinationHasHistory,
        SyncError::DestinationChanged => AbortReason::DestinationChanged,
        SyncError::PublicationAlreadyUsed => AbortReason::PublicationAlreadyUsed,
        SyncError::PreApply(_) => AbortReason::Unauthorized,
        SyncError::Transport(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            AbortReason::Unauthorized
        }
        error => {
            log::debug!("Storage WebSocket sync failed: {error}");
            AbortReason::MalformedMessage
        }
    }
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "sync_authorization_tests.rs"]
mod authorization_tests;
