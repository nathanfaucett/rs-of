//! Transport abstraction for storage client sync.

use alloc::string::String;
use storage_sync_client::transport::TransportError as WsTransportError;

#[cfg(feature = "native")]
use storage_sync_client::transport::native::WebSocketSyncTransport;

#[cfg(feature = "browser")]
use storage_sync_client::browser::browser::BrowserSyncTransport;

/// Transport errors.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Connection closed")]
    Closed,
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Send failed: {0}")]
    SendFailed(String),
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),
    #[error("Timeout")]
    Timeout,
    #[error("TLS error: {0}")]
    TlsError(String),
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
}

impl From<WsTransportError> for TransportError {
    fn from(e: WsTransportError) -> Self {
        match e {
            WsTransportError::Closed => TransportError::Closed,
            WsTransportError::ConnectionFailed(s) => TransportError::ConnectionFailed(s),
            WsTransportError::SendFailed(s) => TransportError::SendFailed(s),
            WsTransportError::ReceiveFailed(s) => TransportError::ReceiveFailed(s),
            WsTransportError::Timeout => TransportError::Timeout,
            WsTransportError::TlsError(s) => TransportError::TlsError(s),
            WsTransportError::WebSocketError(s) => TransportError::WebSocketError(s),
            WsTransportError::Protocol(s) => TransportError::Protocol(s),
        }
    }
}

#[cfg(feature = "native")]
impl From<storage_sync_client::transport::native::TransportError> for TransportError {
    fn from(e: storage_sync_client::transport::native::TransportError) -> Self {
        WsTransportError::from(e).into()
    }
}

#[cfg(feature = "browser")]
impl From<storage_sync_client::browser::browser::TransportError> for TransportError {
    fn from(e: storage_sync_client::browser::browser::TransportError) -> Self {
        match e {
            storage_sync_client::browser::browser::TransportError::Closed => TransportError::Closed,
            storage_sync_client::browser::browser::TransportError::ConnectionFailed(s) => TransportError::ConnectionFailed(s),
            storage_sync_client::browser::browser::TransportError::SendFailed(s) => TransportError::SendFailed(s),
            storage_sync_client::browser::browser::TransportError::ReceiveFailed(s) => TransportError::ReceiveFailed(s),
            storage_sync_client::browser::browser::TransportError::Timeout => TransportError::Timeout,
            storage_sync_client::browser::browser::TransportError::WebSocketError(s) => TransportError::WebSocketError(s),
        }
    }
}

/// Native WebSocket transport wrapper.
#[cfg(feature = "native")]
pub struct NativeWsTransport {
    inner: WebSocketSyncTransport,
}

#[cfg(feature = "native")]
impl NativeWsTransport {
    /// Connect to a WebSocket server.
    pub async fn connect(url: &str) -> Result<Self, TransportError> {
        let inner = WebSocketSyncTransport::connect(url).await?;
        Ok(Self { inner })
    }
}

#[cfg(feature = "native")]
impl storage_sync_client::transport::SyncTransport for NativeWsTransport {
    async fn receive(&mut self) -> Result<Vec<u8>, WsTransportError> {
        self.inner.receive().await
    }

    async fn send(&mut self, payload: Vec<u8>) -> Result<(), WsTransportError> {
        self.inner.send(payload).await
    }

    async fn close(&mut self) -> Result<(), WsTransportError> {
        self.inner.close().await
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }
}

/// Browser WebSocket transport wrapper.
#[cfg(feature = "browser")]
pub struct BrowserWsTransport {
    inner: BrowserSyncTransport,
}

#[cfg(feature = "browser")]
impl BrowserWsTransport {
    /// Connect to a WebSocket server.
    pub async fn connect(url: &str, token: &str) -> Result<Self, TransportError> {
        let inner = BrowserSyncTransport::connect(url, token).await?;
        Ok(Self { inner })
    }
}

#[cfg(feature = "browser")]
impl storage_sync_client::transport::SyncTransport for BrowserWsTransport {
    async fn receive(&mut self) -> Result<Vec<u8>, WsTransportError> {
        self.inner.receive().await.map_err(Into::into)
    }

    async fn send(&mut self, payload: Vec<u8>) -> Result<(), WsTransportError> {
        self.inner.send(payload).await.map_err(Into::into)
    }

    async fn close(&mut self) -> Result<(), WsTransportError> {
        self.inner.close().await.map_err(Into::into)
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }
}

/// Transport configuration.
#[derive(Clone, Debug)]
pub struct TransportConfig {
    pub sync_ws_url: String,
    pub auth_token: String,
}

impl TransportConfig {
    pub fn new(sync_ws_url: String, auth_token: String) -> Self {
        Self {
            sync_ws_url,
            auth_token,
        }
    }
}

/// Re-export the platform-specific transport trait.
pub use storage_sync_client::transport::SyncTransport as WsSyncTransport;
