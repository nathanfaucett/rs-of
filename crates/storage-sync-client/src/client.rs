//! High-level sync client for connecting to the sync server.

use crate::protocol::*;
use crate::transport::{SyncTransport as WsSyncTransport, TransportError};
use ofdb_sql_sync::{
    SessionConfig, SyncResult, SyncRole, SyncTransport,
};
use std::{future::Future, pin::Pin, sync::{Arc, Mutex}, vec::Vec};

use thiserror::Error;
use uuid::Uuid;

#[cfg(feature = "browser")]
use crate::browser::browser::TransportError as BrowserTransportError;

/// Sync client errors.
#[derive(Debug, Error)]
pub enum SyncError {
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("sync session failed: {0}")]
    Session(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("invalid store identity")]
    InvalidStoreIdentity,
    #[error("ofdb sync error: {0}")]
    OfdbSync(String),
}

#[cfg(feature = "browser")]
impl From<BrowserTransportError> for SyncError {
    fn from(e: BrowserTransportError) -> Self {
        SyncError::Protocol(e.to_string())
    }
}

/// Token provider for authentication.
/// Called before each sync session to get a fresh IdP token.
pub type TokenProvider =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<String, SyncError>> + Send>> + Send + Sync>;

/// WebSocket transport bridge for ofdb-sql sync engine.
/// This implements the ofdb-sql-sync SyncTransport trait.
pub struct WsTransportBridge<T: WsSyncTransport> {
    transport: T,
    frame_decoder: FrameDecoder,
}

impl<T: WsSyncTransport> WsTransportBridge<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            frame_decoder: FrameDecoder::new(),
        }
    }
}

impl<T: WsSyncTransport> SyncTransport for WsTransportBridge<T> {
    type Error = TransportError;

    fn receive(&mut self) -> impl Future<Output = Result<Vec<u8>, Self::Error>> {
        let transport = &mut self.transport;
        let decoder = &mut self.frame_decoder;

        async move {
            loop {
                let frame_bytes = transport.receive().await?;
                let frames = decoder.feed(&frame_bytes).map_err(|e| TransportError::Protocol(e.to_string()))?;
                for frame in frames {
                    return Ok(frame.payload);
                }
            }
        }
    }

    fn send(&mut self, payload: Vec<u8>) -> impl Future<Output = Result<(), Self::Error>> {
        let transport = &mut self.transport;
        async move {
            let frame = Frame::new(0, payload, false);
            let frame_bytes = FrameCodec::encode(&frame).map_err(|e| TransportError::Protocol(e.to_string()))?;
            transport.send(frame_bytes).await
        }
    }
}

/// Sync client for native (tokio) environments.
pub struct SyncClient<T: WsSyncTransport> {
    transport: Arc<Mutex<T>>,
    local_replica_id: Uuid,
    token_provider: TokenProvider,
}

impl<T: WsSyncTransport> SyncClient<T> {
    pub fn new(transport: T, local_replica_id: Uuid, token_provider: TokenProvider) -> Self {
        Self {
            transport: Arc::new(Mutex::new(transport)),
            local_replica_id,
            token_provider,
        }
    }

    pub async fn sync(
        &mut self,
        database: &ofdb_sql::Database,
        store_identity: StoreIdentity,
    ) -> Result<SyncResult, SyncError> {
        let token = (self.token_provider)().await?;

        // Send Hello
        let hello = Hello {
            protocol_version: 1,
            manifest: Vec::new(),
            store_identity: store_identity.clone(),
            supported_protocols: vec![ProtocolId::OfdbSqlV7],
        };
        self.send_message(SyncMessage::Hello(hello)).await?;

        // Send AttachRequest
        let attach_req = AttachRequest {
            store_identity,
            auth_token: AuthToken(token.into_bytes()),
            direction: AttachDirection::Bidirectional,
            reconnect_token: None,
        };
        self.send_message(SyncMessage::AttachRequest(attach_req))
            .await?;

        // Receive AttachResponse
        let response = self.receive_message().await?;
        let attach_resp = match response {
            SyncMessage::AttachResponse(r) => r,
            SyncMessage::Abort(reason) => {
                return Err(SyncError::Auth(format!("Attach rejected: {:?}", reason)));
            }
            _ => return Err(SyncError::Protocol("Expected AttachResponse".into())),
        };

        if !attach_resp.accepted {
            return Err(SyncError::Auth("Attach not accepted".into()));
        }

        // Take the transport for sync
        let transport = Arc::clone(&self.transport);

        // Create transport bridge that shares the transport
        let mut transport = WsTransportBridgeShared::new(transport);

        // Configure sync session - all transport is bidirectional
        let config = SessionConfig::default();

        // Run sync session as Initiator
        let result = database
            .synchronize_checked(&mut transport, &config, SyncRole::Initiator, || async {
                Ok(())
            })
            .await
            .map_err(|e| SyncError::OfdbSync(e.to_string()))?;

        Ok(result)
    }

    async fn send_message(&self, msg: SyncMessage) -> Result<(), SyncError> {
        let encoded = msg
            .encode()
            .map_err(|e| SyncError::Protocol(e.to_string()))?;
        let frame = Frame::new(0, encoded, false);
        let frame_bytes =
            FrameCodec::encode(&frame).map_err(|e| SyncError::Protocol(e.to_string()))?;

        let mut guard = self.transport.lock().unwrap();
        guard
            .send(frame_bytes)
            .await
            .map_err(|e| SyncError::Transport(e))?;
        Ok(())
    }

    async fn receive_message(&self) -> Result<SyncMessage, SyncError> {
        let mut guard = self.transport.lock().unwrap();
        let frame_bytes = guard
            .receive()
            .await
            .map_err(|e| SyncError::Transport(e))?;
        let frame =
            FrameCodec::decode(&frame_bytes).map_err(|e| SyncError::Protocol(e.to_string()))?;
        let msg = SyncMessage::decode(&frame.payload)
            .map_err(|e| SyncError::Protocol(e.to_string()))?;
        Ok(msg)
    }
}

/// Shared transport wrapper for SyncTransport trait.
pub struct WsTransportBridgeShared<T: WsSyncTransport> {
    transport: Arc<Mutex<T>>,
}

impl<T: WsSyncTransport> WsTransportBridgeShared<T> {
    fn new(transport: Arc<Mutex<T>>) -> Self {
        Self { transport }
    }
}

impl<T: WsSyncTransport> SyncTransport for WsTransportBridgeShared<T> {
    type Error = TransportError;

    fn receive(&mut self) -> impl Future<Output = Result<Vec<u8>, Self::Error>> {
        let transport = Arc::clone(&self.transport);
        async move {
            let mut guard = transport.lock().unwrap();
            guard.receive().await
        }
    }

    fn send(&mut self, payload: Vec<u8>) -> impl Future<Output = Result<(), Self::Error>> {
        let transport = Arc::clone(&self.transport);
        async move {
            let frame = Frame::new(0, payload, false);
            let frame_bytes = FrameCodec::encode(&frame).map_err(|e| TransportError::Protocol(e.to_string()))?;
            let mut guard = transport.lock().unwrap();
            guard.send(frame_bytes).await
        }
    }
}
