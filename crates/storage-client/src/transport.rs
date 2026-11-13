use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};
use core::{future::Future, pin::Pin};

use futures_util::{SinkExt, StreamExt};

use crate::{
    error::{Result, StorageClientError},
    protocol::{
        AttachDirection, AttachRequest, AuthToken, FRAMING_VERSION, Frame, FrameCodec, Hello,
        MAX_FRAME_PAYLOAD, MAX_SESSION_BYTES, ProtocolId, StoreIdentity, SyncMessage,
    },
};

#[cfg(not(target_arch = "wasm32"))]
pub type TokenFuture = Pin<Box<dyn Future<Output = Result<String>> + Send>>;
#[cfg(target_arch = "wasm32")]
pub type TokenFuture = Pin<Box<dyn Future<Output = Result<String>>>>;
#[cfg(not(target_arch = "wasm32"))]
pub type TokenProvider = Arc<dyn Fn() -> TokenFuture + Send + Sync>;
#[cfg(target_arch = "wasm32")]
pub type TokenProvider = Arc<dyn Fn() -> TokenFuture>;

#[derive(Clone)]
pub struct TransportConfig {
    pub sync_ws_url: String,
}
impl TransportConfig {
    pub fn new(sync_ws_url: String) -> Self {
        Self { sync_ws_url }
    }
}
impl core::fmt::Debug for TransportConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TransportConfig")
            .field("sync_ws_url", &self.sync_ws_url)
            .finish_non_exhaustive()
    }
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
type Socket = gloo_net::websocket::futures::WebSocket;

#[cfg(any(
    all(feature = "native", not(target_arch = "wasm32")),
    all(feature = "browser", target_arch = "wasm32")
))]
pub struct OwnerTransport {
    socket: Socket,
    bytes: u64,
    max_frame: u32,
    max_session: u64,
}

#[cfg(any(
    all(feature = "native", not(target_arch = "wasm32")),
    all(feature = "browser", target_arch = "wasm32")
))]
impl OwnerTransport {
    pub async fn connect(url: &str, identity: &StoreIdentity, token: String) -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let socket = {
            let (socket, _) = tokio_tungstenite::connect_async_with_config(
                url,
                Some(
                    tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
                        .max_message_size(Some(MAX_FRAME_PAYLOAD as usize + 10))
                        .max_frame_size(Some(MAX_FRAME_PAYLOAD as usize + 10)),
                ),
                false,
            )
            .await
            .map_err(|e| StorageClientError::Transport(e.to_string()))?;
            socket
        };
        #[cfg(target_arch = "wasm32")]
        let socket = Socket::open(url).map_err(|e| StorageClientError::Transport(e.to_string()))?;
        let mut transport = Self {
            socket,
            bytes: 0,
            max_frame: MAX_FRAME_PAYLOAD,
            max_session: MAX_SESSION_BYTES,
        };
        transport
            .send_message(SyncMessage::Hello(Hello {
                protocol_version: 1,
                manifest: Vec::new(),
                store_identity: identity.clone(),
                supported_protocols: alloc::vec![ProtocolId::OfdbSqlV7],
            }))
            .await?;
        match transport.receive_message().await? {
            SyncMessage::Hello(hello)
                if hello.protocol_version == 1
                    && hello.store_identity == *identity
                    && hello.supported_protocols.contains(&ProtocolId::OfdbSqlV7) => {}
            _ => return Err(StorageClientError::Protocol("invalid sync hello".into())),
        }
        transport
            .send_message(SyncMessage::AttachRequest(AttachRequest {
                store_identity: identity.clone(),
                auth_token: AuthToken(token.into_bytes()),
                direction: AttachDirection::Bidirectional,
            }))
            .await?;
        match transport.receive_message().await? {
            SyncMessage::AttachResponse(response)
                if response.accepted
                    && response.granted_direction == AttachDirection::Bidirectional
                    && response.max_frame_bytes > 0
                    && response.max_session_bytes >= transport.bytes =>
            {
                transport.max_frame = response.max_frame_bytes.min(MAX_FRAME_PAYLOAD);
                transport.max_session = response.max_session_bytes.min(MAX_SESSION_BYTES);
            }
            SyncMessage::Abort(reason) => {
                return Err(StorageClientError::Auth(alloc::format!(
                    "sync attach denied: {reason:?}"
                )));
            }
            _ => return Err(StorageClientError::Protocol("sync attach rejected".into())),
        }
        Ok(transport)
    }
    fn account(&mut self, bytes: usize) -> Result<()> {
        self.bytes = self.bytes.saturating_add(bytes as u64);
        if self.bytes > self.max_session {
            return Err(StorageClientError::Protocol(
                "sync session byte limit exceeded".into(),
            ));
        }
        Ok(())
    }
    async fn send_message(&mut self, message: SyncMessage) -> Result<()> {
        let payload = message
            .encode()
            .map_err(|e| StorageClientError::Protocol(e.to_string()))?;
        if payload.len() > self.max_frame as usize {
            return Err(StorageClientError::Protocol(
                "sync frame limit exceeded".into(),
            ));
        }
        let bytes = FrameCodec::encode(&Frame {
            version: FRAMING_VERSION,
            flags: 0,
            payload,
        })
        .map_err(|e| StorageClientError::Protocol(alloc::format!("{e:?}")))?;
        self.account(bytes.len())?;
        #[cfg(not(target_arch = "wasm32"))]
        let message = tokio_tungstenite::tungstenite::Message::Binary(bytes.into());
        #[cfg(target_arch = "wasm32")]
        let message = gloo_net::websocket::Message::Bytes(bytes);
        self.socket
            .send(message)
            .await
            .map_err(|e| StorageClientError::Transport(e.to_string()))
    }
    async fn receive_message(&mut self) -> Result<SyncMessage> {
        loop {
            let message = self
                .socket
                .next()
                .await
                .ok_or_else(|| StorageClientError::Transport("sync socket closed".into()))?
                .map_err(|e| StorageClientError::Transport(e.to_string()))?;
            #[cfg(not(target_arch = "wasm32"))]
            let bytes = match message {
                tokio_tungstenite::tungstenite::Message::Binary(bytes) => bytes.to_vec(),
                tokio_tungstenite::tungstenite::Message::Ping(bytes)
                | tokio_tungstenite::tungstenite::Message::Pong(bytes) => {
                    self.account(bytes.len())?;
                    continue;
                }
                _ => {
                    return Err(StorageClientError::Protocol(
                        "binary sync frame required".into(),
                    ));
                }
            };
            #[cfg(target_arch = "wasm32")]
            let gloo_net::websocket::Message::Bytes(bytes) = message else {
                return Err(StorageClientError::Protocol(
                    "binary sync frame required".into(),
                ));
            };
            self.account(bytes.len())?;
            let frame = FrameCodec::decode(&bytes)
                .map_err(|e| StorageClientError::Protocol(alloc::format!("{e:?}")))?;
            if frame.payload.len() > self.max_frame as usize {
                return Err(StorageClientError::Protocol(
                    "sync frame limit exceeded".into(),
                ));
            }
            return SyncMessage::decode(&frame.payload)
                .map_err(|e| StorageClientError::Protocol(e.to_string()));
        }
    }
}

#[cfg(any(
    all(feature = "native", not(target_arch = "wasm32")),
    all(feature = "browser", target_arch = "wasm32")
))]
impl ofdb_sql::history::SyncTransport for OwnerTransport {
    type Error = StorageClientError;
    async fn receive(&mut self) -> Result<Vec<u8>> {
        loop {
            match self.receive_message().await? {
                SyncMessage::OwnerMessage {
                    protocol_id: ProtocolId::OfdbSqlV7,
                    payload,
                } => return Ok(payload),
                SyncMessage::Ping => self.send_message(SyncMessage::Pong).await?,
                SyncMessage::Abort(reason) => {
                    return Err(StorageClientError::Auth(alloc::format!(
                        "sync aborted: {reason:?}"
                    )));
                }
                _ => {
                    return Err(StorageClientError::Protocol(
                        "unexpected sync owner message".into(),
                    ));
                }
            }
        }
    }
    async fn send(&mut self, payload: Vec<u8>) -> Result<()> {
        self.send_message(SyncMessage::OwnerMessage {
            protocol_id: ProtocolId::OfdbSqlV7,
            payload,
        })
        .await
    }
}
