//! Transport abstraction for sync protocol.

use async_trait::async_trait;
use std::{string::String, vec::Vec};
use thiserror::Error;

/// Transport errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportError {
    #[error("connection closed")]
    Closed,
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("send failed: {0}")]
    SendFailed(String),
    #[error("receive failed: {0}")]
    ReceiveFailed(String),
    #[error("timeout")]
    Timeout,
    #[error("tls error: {0}")]
    TlsError(String),
    #[error("websocket error: {0}")]
    WebSocketError(String),
    #[error("protocol error: {0}")]
    Protocol(String),
}

/// Async transport trait for sync protocol.
/// This trait requires Send + Sync for native use.
/// For browser/WASM, use the browser module directly which implements ofdb-sql's SyncTransport.
#[async_trait]
pub trait SyncTransport: Send + Sync {
    /// Receive a frame payload.
    async fn receive(&mut self) -> Result<Vec<u8>, TransportError>;

    /// Send a frame payload.
    async fn send(&mut self, payload: Vec<u8>) -> Result<(), TransportError>;

    /// Close the transport.
    async fn close(&mut self) -> Result<(), TransportError>;

    /// Check if transport is connected.
    fn is_connected(&self) -> bool;
}

/// Native WebSocket transport using tokio-tungstenite.
#[cfg(feature = "native")]
pub mod native {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpStream;
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
    use tokio_tungstenite::tungstenite::protocol::Message;

    /// Native WebSocket transport using tokio-tungstenite.
    pub struct WebSocketSyncTransport {
        ws: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        connected: bool,
    }

    impl WebSocketSyncTransport {
        /// Connect to a WebSocket server.
        pub async fn connect(url: &str) -> Result<Self, TransportError> {
            let (ws, _) = connect_async(url)
                .await
                .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;
            Ok(Self {
                ws: Some(ws),
                connected: true,
            })
        }

        /// Create from an existing WebSocket stream.
        pub fn from_stream(ws: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Self {
            Self {
                ws: Some(ws),
                connected: true,
            }
        }
    }

    #[async_trait]
    impl SyncTransport for WebSocketSyncTransport {
        async fn receive(&mut self) -> Result<Vec<u8>, TransportError> {
            let ws = self.ws.as_mut().ok_or(TransportError::Closed)?;
            match ws.next().await {
                Some(Ok(Message::Binary(data))) => Ok(data.to_vec()),
                Some(Ok(Message::Close(_))) => {
                    self.connected = false;
                    Err(TransportError::Closed)
                }
                Some(Err(e)) => Err(TransportError::ReceiveFailed(e.to_string())),
                Some(Ok(_)) => {
                    // Ignore non-binary messages
                    self.receive().await
                }
                None => {
                    self.connected = false;
                    Err(TransportError::Closed)
                }
            }
        }

        async fn send(&mut self, payload: Vec<u8>) -> Result<(), TransportError> {
            let ws = self.ws.as_mut().ok_or(TransportError::Closed)?;
            ws.send(Message::Binary(payload.into()))
                .await
                .map_err(|e| TransportError::SendFailed(e.to_string()))
        }

        async fn close(&mut self) -> Result<(), TransportError> {
            if let Some(mut ws) = self.ws.take() {
                let _ = ws.close(None).await;
            }
            self.connected = false;
            Ok(())
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }
}

/// Re-export the native transport.
#[cfg(feature = "native")]
pub use native::WebSocketSyncTransport;

#[cfg(test)]
///s*In-memory transport for testing.
pub mod test {
    use super::*;
    use futures_channel::mpsc;
use futures_channel::mpsc::TryRecvError;
    use std::sync::{Arc, Mutex};

    /// Pair of connected in-memory transports for testing.
    pub fn create_transport_pair() -> (MemoryTransport, MemoryTransport) {
        let (tx1, rx1) = mpsc::channel(32);
        let (tx2, rx2) = mpsc::channel(32);

        let shared1 = Arc::new(Mutex::new((tx2, rx1)));
        let shared2 = Arc::new(Mutex::new((tx1, rx2)));

        (
            MemoryTransport {
                inner: shared1,
                connected: true,
            },
            MemoryTransport {
                inner: shared2,
                connected: true,
            },
        )
    }

    pub struct MemoryTransport {
        inner: Arc<Mutex<(mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>)>>,
        connected: bool,
    }

    #[async_trait]
    impl SyncTransport for MemoryTransport {
        async fn receive(&mut self) -> Result<Vec<u8>, TransportError> {
            loop {
                let item = {
                    let mut guard = self.inner.lock().unwrap();
                    guard.1.try_recv()
                };
                match item {
                    Ok(data) => return Ok(data),
                    Err(TryRecvError::Empty) => {
                        // Channel empty, yield and retry
                        std::thread::yield_now();
                        std::future::ready(()).await;
                    }
                    Err(TryRecvError::Closed) => {
                        self.connected = false;
                        return Err(TransportError::Closed);
                    }
                }
            }
        }

        async fn send(&mut self, payload: Vec<u8>) -> Result<(), TransportError> {
            let mut guard = self.inner.lock().unwrap();
            guard
                .0
                .try_send(payload)
                .map_err(|_| TransportError::SendFailed("channel full".into()))
        }

        async fn close(&mut self) -> Result<(), TransportError> {
            self.connected = false;
            Ok(())
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }
}


#[async_trait]
impl<T: SyncTransport + ?Sized> SyncTransport for Box<T> {
    async fn receive(&mut self) -> Result<Vec<u8>, TransportError> {
        (**self).receive().await
    }

    async fn send(&mut self, payload: Vec<u8>) -> Result<(), TransportError> {
        (**self).send(payload).await
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        (**self).close().await
    }

    fn is_connected(&self) -> bool {
        (**self).is_connected()
    }
}
