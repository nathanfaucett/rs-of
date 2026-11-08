//! High-level sync client for connecting to the sync server.

use crate::protocol::*;
use crate::transport::{SyncTransport as WsSyncTransport, TransportError};
use ofdb_sql::{
    SessionConfig, SyncDirection as OfdbSyncDirection, SyncResult, SyncRole, SyncTransport,
};
use std::{future::Future, path::Path, pin::Pin, vec::Vec};

use thiserror::Error;
use uuid::Uuid;

#[cfg(feature = "browser")]
use crate::browser::browser::TransportError as BrowserTransportError;

/// Synchronization direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncDirection {
    Push,
    Pull,
    Bidirectional,
}

impl From<SyncDirection> for OfdbSyncDirection {
    fn from(d: SyncDirection) -> Self {
        match d {
            SyncDirection::Push => OfdbSyncDirection::SendOnly,
            SyncDirection::Pull => OfdbSyncDirection::ReceiveOnly,
            SyncDirection::Bidirectional => OfdbSyncDirection::Bidirectional,
        }
    }
}

impl From<SyncDirection> for AttachDirection {
    fn from(d: SyncDirection) -> Self {
        match d {
            SyncDirection::Push => AttachDirection::Push,
            SyncDirection::Pull => AttachDirection::Pull,
            SyncDirection::Bidirectional => AttachDirection::Bidirectional,
        }
    }
}

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
                let frames = decoder
                    .feed(&frame_bytes)
                    .map_err(|e| TransportError::ReceiveFailed(e.to_string()))?;

                for frame in frames {
                    return Ok(frame.payload);
                }
                // No complete frame yet, continue loop
            }
        }
    }

    fn send(&mut self, payload: Vec<u8>) -> impl Future<Output = Result<(), Self::Error>> {
        let transport = &mut self.transport;

        async move {
            let frame = Frame::new(0, payload, false);
            let frame_bytes = FrameCodec::encode(&frame)
                .map_err(|e| TransportError::SendFailed(e.to_string()))?;
            transport.send(frame_bytes).await
        }
    }
}

/// High-level sync client (native only).
#[cfg(feature = "native")]
pub struct SyncClient<T: WsSyncTransport> {
    transport: T,
    token_provider: TokenProvider,
    store_identity: StoreIdentity,
    local_replica_id: Uuid,
}

#[cfg(feature = "native")]
impl<T: WsSyncTransport> SyncClient<T> {
    /// Create a new sync client.
    ///
    /// # Arguments
    /// * `transport` - Connected WebSocket transport
    /// * `token_provider` - Async function that returns a valid IdP access token for `storage:sync`
    /// * `store_identity` - Stable store identity (store_id + schema fingerprint)
    /// * `local_replica_id` - Unique replica ID for this client instance
    pub fn new(
        transport: T,
        token_provider: TokenProvider,
        store_identity: StoreIdentity,
        local_replica_id: Uuid,
    ) -> Self {
        Self {
            transport,
            token_provider,
            store_identity,
            local_replica_id,
        }
    }

    /// Run a synchronization session with a local database.
    ///
    /// # Arguments
    /// * `direction` - Push, Pull, or Bidirectional
    /// * `database` - Open ofdb-sql Database with sync enabled (requires native-stage feature)
    /// * `_namespace` - Storage namespace for multi-tenancy (currently unused)
    /// * `staging_dir` - Directory for staging sync data
    pub async fn sync(
        mut self,
        direction: SyncDirection,
        database: &ofdb_sql::Database,
        _namespace: &impl storage_model::StorageNamespace,
        staging_dir: impl AsRef<Path>,
    ) -> Result<SyncResult, SyncError> {
        // Get fresh auth token
        let token: String = (self.token_provider)().await?;
        let auth_token = crate::protocol::AuthToken(token.into_bytes());

        // Send Hello
        let hello = Hello {
            protocol_version: 1,
            manifest: Vec::new(),
            store_identity: self.store_identity.clone(),
            supported_protocols: vec![ProtocolId::OfdbSqlV7],
        };
        self.send_message(SyncMessage::Hello(hello)).await?;

        // Send AttachRequest
        let attach_req = AttachRequest {
            store_identity: self.store_identity.clone(),
            auth_token,
            direction: direction.into(),
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

        // Create WebSocket transport bridge for ofdb-sql sync engine
        let mut ws_transport = WsTransportBridge::new(self.transport);

        // Configure sync session
        let mut config = SessionConfig::default();
        config.direction = direction.into();

        // Run sync session as Initiator (client initiates sync)
        let result = database
            .synchronize_with_stage_directory_checked(
                &mut ws_transport,
                &config,
                SyncRole::Initiator,
                staging_dir,
                || async { Ok(()) },
            )
            .await
            .map_err(|e| SyncError::OfdbSync(e.to_string()))?;

        tracing::info!("Sync completed: {:?}", result);
        Ok(result)
    }

    async fn send_message(&mut self, msg: SyncMessage) -> Result<(), SyncError> {
        let encoded = msg
            .encode()
            .map_err(|e| SyncError::Protocol(e.to_string()))?;
        let frame = Frame::new(0, encoded, false);
        let frame_bytes =
            FrameCodec::encode(&frame).map_err(|e| SyncError::Protocol(e.to_string()))?;
        self.transport.send(frame_bytes).await?;
        Ok(())
    }

    async fn receive_message(&mut self) -> Result<SyncMessage, SyncError> {
        let frame_bytes = self.transport.receive().await?;
        let frame =
            FrameCodec::decode(&frame_bytes).map_err(|e| SyncError::Protocol(e.to_string()))?;
        let msg =
            SyncMessage::decode(&frame.payload).map_err(|e| SyncError::Protocol(e.to_string()))?;
        Ok(msg)
    }
}

/// Convenience function for native clients.
#[cfg(feature = "native")]
pub async fn connect_native(
    url: &str,
    token_provider: TokenProvider,
    store_identity: StoreIdentity,
    local_replica_id: Uuid,
) -> Result<SyncClient<crate::transport::native::WebSocketSyncTransport>, SyncError> {
    let transport = crate::transport::native::WebSocketSyncTransport::connect(url).await?;
    Ok(SyncClient::new(
        transport,
        token_provider,
        store_identity,
        local_replica_id,
    ))
}

/// Convenience function for browser clients.
#[cfg(feature = "browser")]
pub async fn connect_browser(
    url: &str,
    token_provider: crate::browser::browser_client::BrowserTokenProvider,
    store_identity: StoreIdentity,
    local_replica_id: Uuid,
) -> Result<crate::browser::browser_client::BrowserSyncClient, crate::SyncError> {
    crate::browser::browser_client::BrowserSyncClient::new(
        url,
        token_provider,
        store_identity,
        local_replica_id,
    )
    .await
}

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;
    use crate::transport::test::create_transport_pair;

    #[tokio::test]
    async fn client_roundtrip() {
        let (client_transport, mut server_transport) = create_transport_pair();

        let store_identity = StoreIdentity {
            store_id: Uuid::now_v7(),
            replica_id: Uuid::now_v7(),
            creation_epoch: 1,
            schema_fingerprint: [0u8; 32],
        };
        let local_replica_id = Uuid::now_v7();

        let token_provider: TokenProvider =
            Box::new(|| Box::pin(async { Ok::<_, SyncError>("test-token".to_string()) }));

        let mut client = SyncClient::new(
            client_transport,
            token_provider,
            store_identity.clone(),
            local_replica_id,
        );

        // Test Hello exchange
        let hello = Hello {
            protocol_version: 1,
            manifest: Vec::new(),
            store_identity: store_identity.clone(),
            supported_protocols: vec![ProtocolId::OfdbSqlV7],
        };
        client
            .send_message(SyncMessage::Hello(hello))
            .await
            .unwrap();

        // Server receives and responds
        let frame_bytes = server_transport.receive().await.unwrap();
        let frame = FrameCodec::decode(&frame_bytes).unwrap();
        let msg = SyncMessage::decode(&frame.payload).unwrap();

        match msg {
            SyncMessage::Hello(h) => {
                assert_eq!(h.store_identity.store_id, store_identity.store_id);
            }
            _ => panic!("Expected Hello"),
        }
    }
}
