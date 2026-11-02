//! Transport abstraction for sync protocol.

use std::{string::String, vec::Vec};
use async_trait::async_trait;
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
}

/// Async transport trait for sync protocol.
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

/// WebSocket-based sync transport for browser and native.
#[cfg(feature = "native")]
pub mod native {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use tokio::net::TcpStream;
    use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
    use tungstenite::protocol::Message;

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
                Some(Ok(Message::Binary(data))) => Ok(data.as_slice().to_vec()),
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

/// Browser WebSocket transport using web-sys.
#[cfg(feature = "browser")]
pub mod browser {
    use super::*;
    use futures::channel::mpsc;
    use futures::StreamExt;
    use gloo_net::websocket::{futures::WebSocket, Message};
    use wasm_bindgen::JsValue;

    /// Browser WebSocket transport using gloo-net.
    pub struct WebSocketSyncTransport {
        ws: Option<WebSocket>,
        rx: Option<mpsc::Receiver<Result<Vec<u8>, TransportError>>>,
        connected: bool,
    }

    impl WebSocketSyncTransport {
        /// Connect to a WebSocket server.
        pub async fn connect(url: &str) -> Result<Self, TransportError> {
            let ws = WebSocket::open(url).map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;
            let (tx, rx) = mpsc::channel(32);

            let ws_clone = ws.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let mut stream = ws_clone.into_future();
                while let Some(msg) = stream.next().await {
                    match msg {
                        Ok(Message::Bytes(data)) => {
                            if tx.send(Ok(data)).await.is_err() {
                                break;
                            }
                        }
                        Ok(Message::Text(_)) => {
                            // Ignore text messages
                        }
                        Err(e) => {
                            let _ = tx.send(Err(TransportError::ReceiveFailed(e.to_string()))).await;
                            break;
                        }
                    }
                }
            });

            Ok(Self {
                ws: Some(ws),
                rx: Some(rx),
                connected: true,
            })
        }
    }

    #[async_trait]
    impl SyncTransport for WebSocketSyncTransport {
        async fn receive(&mut self) -> Result<Vec<u8>, TransportError> {
            let rx = self.rx.as_mut().ok_or(TransportError::Closed)?;
            match rx.next().await {
                Some(Ok(data)) => Ok(data),
                Some(Err(e)) => Err(e),
                None => {
                    self.connected = false;
                    Err(TransportError::Closed)
                }
            }
        }

        async fn send(&mut self, payload: Vec<u8>) -> Result<(), TransportError> {
            let ws = self.ws.as_ref().ok_or(TransportError::Closed)?;
            ws.send_binary(&payload)
                .map_err(|e| TransportError::SendFailed(e.to_string()))
        }

        async fn close(&mut self) -> Result<(), TransportError> {
            if let Some(ws) = self.ws.take() {
                let _ = ws.close();
            }
            self.connected = false;
            Ok(())
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }
}

/// Re-export the appropriate transport based on features.
#[cfg(feature = "native")]
pub use native::WebSocketSyncTransport;

#[cfg(feature = "browser")]
pub use browser::WebSocketSyncTransport;

/// In-memory transport for testing.
pub mod test {
    use super::*;
    use futures::channel::mpsc;
    use std::sync::{Arc, Mutex};

    /// Pair of connected in-memory transports for testing.
    pub fn create_transport_pair() -> (MemoryTransport, MemoryTransport) {
        let (tx1, rx1) = mpsc::channel(32);
        let (tx2, rx2) = mpsc::channel(32);

        let shared1 = Arc::new(Mutex::new((tx2, rx1)));
        let shared2 = Arc::new(Mutex::new((tx1, rx2)));

        (
            MemoryTransport { inner: shared1, connected: true },
            MemoryTransport { inner: shared2, connected: true },
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
                    Err(futures::channel::mpsc::TryRecvError::Empty) => {
                        // Channel empty, yield and retry
                        std::thread::yield_now();
                        futures::future::ready(()).await;
                    }
                    Err(futures::channel::mpsc::TryRecvError::Closed) => {
                        self.connected = false;
                        return Err(TransportError::Closed);
                    }
                }
            }
        }

        async fn send(&mut self, payload: Vec<u8>) -> Result<(), TransportError> {
            let mut guard = self.inner.lock().unwrap();
            guard.0.try_send(payload).map_err(|_| TransportError::SendFailed("channel full".into()))
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
