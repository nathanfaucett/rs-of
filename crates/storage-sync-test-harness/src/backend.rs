//! Test backend implementation for sync protocol.

use std::collections::BTreeMap;
use std::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::frames::{MAX_FRAME_PAYLOAD, MAX_SESSION_BYTES};
#[cfg(feature = "native")]
use crate::frames::Frame;
use crate::protocol::*;

/// Test backend configuration.
#[derive(Clone, Debug)]
pub struct BackendConfig {
    pub max_frame_bytes: u32,
    pub max_session_bytes: u64,
    pub supported_protocols: Vec<ProtocolId>,
    pub require_auth: bool,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            max_frame_bytes: MAX_FRAME_PAYLOAD,
            max_session_bytes: MAX_SESSION_BYTES,
            supported_protocols: vec![ProtocolId::OfdbSqlV7],
            require_auth: true,
        }
    }
}

/// Store registration in backend.
#[derive(Clone, Debug)]
struct RegisteredStore {
    _store_identity: StoreIdentity,
    auth_token: AuthToken,
    granted_direction: AttachDirection,
    backend_replica_id: Uuid,
    session_bytes_sent: u64,
    session_bytes_received: u64,
    last_ack: Option<Acknowledgement>,
    reconnect_token: Option<ReconnectToken>,
    revoked: bool,
}

/// Test backend for sync protocol.
pub struct TestBackend {
    config: BackendConfig,
    stores: Arc<Mutex<BTreeMap<Uuid, RegisteredStore>>>, // Keyed by store_id
    sessions: Arc<Mutex<BTreeMap<Uuid, Uuid>>>, // session_id -> store_id
    next_session_id: AtomicU64,
}

impl TestBackend {
    pub fn new(config: BackendConfig) -> Self {
        Self {
            config,
            stores: Arc::new(Mutex::new(BTreeMap::new())),
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
            next_session_id: AtomicU64::new(1),
        }
    }

    /// Register a store with the backend (simulates enrollment).
    pub fn register_store(
        &self,
        store_identity: StoreIdentity,
        auth_token: AuthToken,
        direction: AttachDirection,
    ) -> Uuid {
        let backend_replica_id = Uuid::new_v4();
        let store = RegisteredStore {
            _store_identity: store_identity.clone(),
            auth_token,
            granted_direction: direction,
            backend_replica_id,
            session_bytes_sent: 0,
            session_bytes_received: 0,
            last_ack: None,
            reconnect_token: None,
            revoked: false,
        };
        self.stores.lock().unwrap().insert(store_identity.store_id, store);
        backend_replica_id
    }

    /// Revoke a store's authorization.
    pub fn revoke_store(&self, store_id: Uuid) -> bool {
        if let Some(store) = self.stores.lock().unwrap().get_mut(&store_id) {
            store.revoked = true;
            true
        } else {
            false
        }
    }

    /// Run the backend loop accepting connections.
    #[cfg(feature = "native")]
    pub async fn run(&self, addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use tokio::net::TcpListener;
        use tokio_tungstenite::accept_async;

        let listener = TcpListener::bind(addr).await?;
        tracing::info!("Test backend listening on {}", addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let backend = self.clone();
            tokio::spawn(async move {
                if let Err(e) = backend.handle_connection(stream).await {
                    tracing::warn!("Connection error: {}", e);
                }
            });
        }
    }

    /// Handle a single WebSocket connection.
    #[cfg(feature = "native")]
    async fn handle_connection(
        &self,
        stream: tokio::net::TcpStream,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use futures::{SinkExt, StreamExt};
        use tokio_tungstenite::{accept_async, WebSocketStream};
        use tungstenite::protocol::Message;

        let ws = accept_async(stream).await?;
        let (mut ws_sender, mut ws_receiver) = ws.split();

        let mut frame_decoder = crate::frames::FrameDecoder::new();
        let mut session_state = SessionState::new();

        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    let frames = frame_decoder.feed(data.as_slice())?;
                    for frame in frames {
                        if let Some(response_frames) = self.process_frame(frame, &mut session_state)? {
                            for resp_frame in response_frames {
                                let encoded = FrameCodec::encode(&resp_frame)?;
                                ws_sender.send(Message::Binary(encoded.into())).await?;
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
        }

        Ok(())
    }

    /// Process a single frame, returning optional response frames.
    #[cfg(feature = "native")]
    fn process_frame(
        &self,
        frame: Frame,
        state: &mut SessionState,
    ) -> Result<Option<Vec<Frame>>, Box<dyn std::error::Error + Send + Sync>> {
        let message = SyncMessage::decode(&frame.payload)?;

        match message {
            SyncMessage::Hello(hello) => {
                let response = self.handle_hello(hello, state)?;
                Ok(Some(vec![Frame::new(0, response.encode()?, false)]))
            }
            SyncMessage::AttachRequest(attach) => {
                let response = self.handle_attach(attach, state)?;
                Ok(Some(vec![Frame::new(0, response.encode()?, false)]))
            }
            SyncMessage::OwnerMessage { protocol_id, payload } => {
                self.handle_owner_message(protocol_id, payload, state)?;
                Ok(None)
            }
            SyncMessage::OwnerMessageBatch { protocol_id, payloads } => {
                for payload in payloads {
                    self.handle_owner_message(protocol_id.clone(), payload, state)?;
                }
                Ok(None)
            }
            SyncMessage::Ack(ack) => {
                self.handle_ack(ack, state);
                Ok(None)
            }
            SyncMessage::Cancel(reason) => {
                let response = self.handle_cancel(reason, state)?;
                Ok(Some(vec![Frame::new(0, response.encode()?, false)]))
            }
            SyncMessage::Ping => {
                Ok(Some(vec![Frame::new(0, SyncMessage::Pong.encode()?, false)]))
            }
            _ => {
                // Unsupported message type
                let abort = SyncMessage::Abort(AbortReason::ProtocolError(
                    "unsupported message type".into(),
                ));
                Ok(Some(vec![Frame::new(0, abort.encode()?, false)]))
            }
        }
    }

    pub fn handle_hello(&self, hello: Hello, state: &mut SessionState) -> Result<SyncMessage, Box<dyn std::error::Error + Send + Sync>> {
        // Check protocol version
        if hello.protocol_version != 1 {
            return Ok(SyncMessage::Abort(AbortReason::VersionMismatch));
        }

        // Check supported protocols
        let mut common_protocol = None;
        for proto in &hello.supported_protocols {
            if self.config.supported_protocols.contains(proto) {
                common_protocol = Some(proto.clone());
                break;
            }
        }

        if common_protocol.is_none() {
            return Ok(SyncMessage::Abort(AbortReason::IncompatibleProtocol));
        }

        state.peer_identity = Some(hello.store_identity.clone());
        state.negotiated_protocol = common_protocol;
        state.protocol_version = hello.protocol_version;

        // Respond with our hello
        Ok(SyncMessage::Hello(Hello {
            protocol_version: 1,
            manifest: Vec::new(), // Backend doesn't have a manifest
            store_identity: StoreIdentity {
                store_id: Uuid::nil(), // Backend doesn't have a store_id
                replica_id: Uuid::new_v4(),
                creation_epoch: 0,
                schema_fingerprint: [0u8; 32],
            },
            supported_protocols: self.config.supported_protocols.clone(),
        }))
    }

    pub fn handle_attach(
        &self,
        attach: AttachRequest,
        state: &mut SessionState,
    ) -> Result<SyncMessage, Box<dyn std::error::Error + Send + Sync>> {
        let peer_identity = state.peer_identity.as_ref().ok_or("no hello received")?;

        // Verify store identity matches hello
        if attach.store_identity.store_id != peer_identity.store_id
            || attach.store_identity.replica_id != peer_identity.replica_id
        {
            return Ok(SyncMessage::Abort(AbortReason::ProtocolError(
                "store identity mismatch".into(),
            )));
        }

        // Check if store is registered
        let mut stores = self.stores.lock().unwrap();
        let store = stores.get_mut(&attach.store_identity.store_id).ok_or("store not found")?;

        // Verify auth token
        if self.config.require_auth && store.auth_token != attach.auth_token {
            return Ok(SyncMessage::Abort(AbortReason::Unauthorized));
        }

        // Check revocation
        if store.revoked {
            return Ok(SyncMessage::Abort(AbortReason::AuthorizationRevoked));
        }

        // Check direction
        if !direction_granted(store.granted_direction, attach.direction) {
            return Ok(SyncMessage::Abort(AbortReason::DirectionNotGranted));
        }

        // Handle reconnect
        if let Some(token) = attach.reconnect_token {
            // Verify token matches stored state
            if let Some(stored_token) = &store.reconnect_token {
                if token.session_id != stored_token.session_id {
                    return Ok(SyncMessage::Abort(AbortReason::ProtocolError(
                        "invalid reconnect token".into(),
                    )));
                }
                state.resumed = true;
                state.resume_from_bytes = token.last_acknowledged_bytes;
            }
        }

        // Create new session
        let session_id = Uuid::from_u64_pair(0, self.next_session_id.fetch_add(1, Ordering::SeqCst));
        state.session_id = Some(session_id);
        state.store_id = Some(attach.store_identity.store_id);
        state.direction = attach.direction;

        // Register session
        self.sessions.lock().unwrap().insert(session_id, attach.store_identity.store_id);

        // Update store session state
        store.session_bytes_sent = 0;
        store.session_bytes_received = 0;
        store.last_ack = None;

        Ok(SyncMessage::AttachResponse(AttachResponse {
            accepted: true,
            backend_replica_id: store.backend_replica_id,
            session_id,
            granted_direction: store.granted_direction,
            max_frame_bytes: self.config.max_frame_bytes,
            max_session_bytes: self.config.max_session_bytes,
        }))
    }

    #[cfg(feature = "native")]
    fn handle_owner_message(
        &self,
        _protocol_id: ProtocolId,
        payload: Vec<u8>,
        state: &mut SessionState,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // In a real backend, this would forward to the sql-sync engine
        // For test harness, we just track bytes and simulate processing
        if let Some(store_id) = state.store_id {
            let mut stores = self.stores.lock().unwrap();
            if let Some(store) = stores.get_mut(&store_id) {
                store.session_bytes_received += payload.len() as u64;
                if store.session_bytes_received > self.config.max_session_bytes {
                    return Err("session too large".into());
                }
            }
        }
        Ok(())
    }

    #[cfg(feature = "native")]
    fn handle_ack(&self, ack: Acknowledgement, state: &mut SessionState) {
        if let Some(store_id) = state.store_id {
            let mut stores = self.stores.lock().unwrap();
            if let Some(store) = stores.get_mut(&store_id) {
                store.last_ack = Some(ack.clone());
                // Generate reconnect token
                store.reconnect_token = Some(ReconnectToken {
                    session_id: state.session_id.unwrap_or(Uuid::nil()),
                    last_acknowledged_bytes: ack.acknowledged_bytes,
                    last_acknowledged_units: ack.acknowledged_units,
                    sender_state_hash: [0u8; 32], // Would be computed from store state
                    receiver_state_hash: ack.receiver_state_hash,
                    timestamp_ms: current_timestamp_ms(),
                });
            }
        }
    }

    #[cfg(feature = "native")]
    fn handle_cancel(
        &self,
        reason: CancelReason,
        state: &mut SessionState,
    ) -> Result<SyncMessage, Box<dyn std::error::Error + Send + Sync>> {
        // Clean up session
        if let Some(session_id) = state.session_id {
            self.sessions.lock().unwrap().remove(&session_id);
        }
        state.reset();

        Ok(SyncMessage::Abort(AbortReason::ProtocolError(format!(
            "cancelled: {:?}",
            reason
        ))))
    }

    /// Clone for use in async tasks.
    #[cfg(feature = "native")]
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            stores: self.stores.clone(),
            sessions: self.sessions.clone(),
            next_session_id: AtomicU64::new(
                self.next_session_id.load(Ordering::SeqCst),
            ),
        }
    }
}

/// Session state during connection handling.
pub struct SessionState {
    peer_identity: Option<StoreIdentity>,
    negotiated_protocol: Option<ProtocolId>,
    protocol_version: u16,
    session_id: Option<Uuid>,
    store_id: Option<Uuid>,
    direction: AttachDirection,
    resumed: bool,
    resume_from_bytes: u64,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            peer_identity: None,
            negotiated_protocol: None,
            protocol_version: 0,
            session_id: None,
            store_id: None,
            direction: AttachDirection::Bidirectional,
            resumed: false,
            resume_from_bytes: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

fn direction_granted(granted: AttachDirection, requested: AttachDirection) -> bool {
    match (granted, requested) {
        (AttachDirection::Bidirectional, _) => true,
        (AttachDirection::Push, AttachDirection::Push) => true,
        (AttachDirection::Pull, AttachDirection::Pull) => true,
        _ => false,
    }
}

#[cfg(feature = "native")]
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_register_and_attach() {
        let backend = TestBackend::new(BackendConfig::default());
        let store_id = Uuid::new_v4();
        let replica_id = Uuid::new_v4();
        let auth_token = AuthToken(b"test-token".to_vec());

        let identity = StoreIdentity {
            store_id,
            replica_id,
            creation_epoch: 1,
            schema_fingerprint: [0u8; 32],
        };

        backend.register_store(identity.clone(), auth_token.clone(), AttachDirection::Bidirectional);

        // Simulate hello + attach
        let hello = Hello {
            protocol_version: 1,
            manifest: vec![],
            store_identity: identity.clone(),
            supported_protocols: vec![ProtocolId::OfdbSqlV7],
        };

        let mut state = SessionState::new();
        let response = backend.handle_hello(hello, &mut state).unwrap();
        assert!(matches!(response, SyncMessage::Hello(_)));

        let attach = AttachRequest {
            store_identity: identity,
            auth_token,
            direction: AttachDirection::Bidirectional,
            reconnect_token: None,
        };

        let response = backend.handle_attach(attach, &mut state).unwrap();
        match response {
            SyncMessage::AttachResponse(resp) => {
                assert!(resp.accepted);
                assert_eq!(resp.granted_direction, AttachDirection::Bidirectional);
            }
            _ => panic!("expected attach response"),
        }
    }

    #[test]
    fn backend_rejects_invalid_auth() {
        let backend = TestBackend::new(BackendConfig::default());
        let store_id = Uuid::new_v4();
        let replica_id = Uuid::new_v4();

        let identity = StoreIdentity {
            store_id,
            replica_id,
            creation_epoch: 1,
            schema_fingerprint: [0u8; 32],
        };

        backend.register_store(
            identity.clone(),
            AuthToken(b"correct-token".to_vec()),
            AttachDirection::Bidirectional,
        );

        let mut state = SessionState::new();
        let hello = Hello {
            protocol_version: 1,
            manifest: vec![],
            store_identity: identity.clone(),
            supported_protocols: vec![ProtocolId::OfdbSqlV7],
        };
        backend.handle_hello(hello, &mut state).unwrap();

        let attach = AttachRequest {
            store_identity: identity,
            auth_token: AuthToken(b"wrong-token".to_vec()),
            direction: AttachDirection::Bidirectional,
            reconnect_token: None,
        };

        let response = backend.handle_attach(attach, &mut state).unwrap();
        assert!(matches!(response, SyncMessage::Abort(AbortReason::Unauthorized)));
    }

    #[test]
    fn backend_revocation() {
        let backend = TestBackend::new(BackendConfig::default());
        let store_id = Uuid::new_v4();
        let replica_id = Uuid::new_v4();
        let auth_token = AuthToken(b"test-token".to_vec());

        let identity = StoreIdentity {
            store_id,
            replica_id,
            creation_epoch: 1,
            schema_fingerprint: [0u8; 32],
        };

        backend.register_store(identity.clone(), auth_token.clone(), AttachDirection::Bidirectional);
        backend.revoke_store(store_id);

        let mut state = SessionState::new();
        let hello = Hello {
            protocol_version: 1,
            manifest: vec![],
            store_identity: identity.clone(),
            supported_protocols: vec![ProtocolId::OfdbSqlV7],
        };
        backend.handle_hello(hello, &mut state).unwrap();

        let attach = AttachRequest {
            store_identity: identity,
            auth_token,
            direction: AttachDirection::Bidirectional,
            reconnect_token: None,
        };

        let response = backend.handle_attach(attach, &mut state).unwrap();
        assert!(matches!(response, SyncMessage::Abort(AbortReason::AuthorizationRevoked)));
    }
}
