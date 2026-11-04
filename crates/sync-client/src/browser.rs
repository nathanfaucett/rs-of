//! Browser-specific sync client implementation using web-sys directly.
//! This avoids Send + Sync requirements by implementing ofdb-sql's SyncTransport directly.

#[cfg(feature = "browser")]
use crate::protocol::FrameDecoder;
#[cfg(feature = "browser")]
use futures_channel::mpsc;
#[cfg(feature = "browser")]
use futures_util::StreamExt;
#[cfg(feature = "browser")]
use ofdb_sql::SyncTransport;
#[cfg(feature = "browser")]
use std::cell::RefCell;
#[cfg(feature = "browser")]
use std::future::Future;
#[cfg(feature = "browser")]
use std::pin::Pin;
#[cfg(feature = "browser")]
use std::rc::Rc;
#[cfg(feature = "browser")]
use wasm_bindgen::JsCast;
#[cfg(feature = "browser")]
use web_sys::js_sys::{ArrayBuffer, Uint8Array};
#[cfg(feature = "browser")]
use web_sys::{BinaryType, MessageEvent, WebSocket};

#[cfg(feature = "browser")]
pub mod browser {
    use super::*;

    /// Browser WebSocket transport using web-sys directly.
    /// Implements ofdb-sql's SyncTransport trait (no Send + Sync required).
    #[derive(Clone)]
    pub struct BrowserSyncTransport {
        ws: Rc<RefCell<Option<WebSocket>>>,
        rx: Rc<RefCell<Option<mpsc::Receiver<Result<Vec<u8>, TransportError>>>>>,
        frame_decoder: Rc<RefCell<FrameDecoder>>,
        connected: Rc<RefCell<bool>>,
    }

    #[derive(Debug, thiserror::Error, Clone)]
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
        #[error("websocket error: {0}")]
        WebSocketError(String),
    }

    impl BrowserSyncTransport {
        /// Connect to a WebSocket server.
        pub fn connect(
            url: &str,
            token: &str,
        ) -> impl Future<Output = Result<Self, TransportError>> {
            let url = format!("{}?token={}", url, token);
            let (tx, rx) = mpsc::channel(32);
            let rx = Rc::new(RefCell::new(Some(rx)));
            let ws = Rc::new(RefCell::new(None));
            let frame_decoder = Rc::new(RefCell::new(FrameDecoder::new()));
            let connected = Rc::new(RefCell::new(false));

            let ws_clone = ws.clone();
            let rx_clone = rx.clone();
            let frame_decoder_clone = frame_decoder.clone();
            let connected_clone = connected.clone();
            let mut tx_onmessage = tx.clone();
            let mut tx_onerror = tx.clone();
            let mut tx_onclose = tx.clone();

            async move {
                let ws_obj = WebSocket::new(&url)
                    .map_err(|e| TransportError::ConnectionFailed(format!("{:?}", e)))?;
                ws_obj.set_binary_type(BinaryType::Arraybuffer);

                let ws_obj_clone = ws_obj.clone();
                let onmessage_cb =
                    wasm_bindgen::closure::Closure::wrap(Box::new(move |e: MessageEvent| {
                        if let Ok(array_buffer) = e.data().dyn_into::<ArrayBuffer>() {
                            let uint8_array = Uint8Array::new(&array_buffer);
                            let mut data = vec![0u8; uint8_array.length() as usize];
                            uint8_array.copy_to(&mut data);

                            let mut decoder = frame_decoder_clone.borrow_mut();
                            if let Ok(frames) = decoder.feed(&data) {
                                for frame in frames {
                                    if let Err(_) = tx_onmessage.try_send(Ok(frame.payload)) {
                                        // Channel full, drop frame
                                    }
                                }
                            }
                        }
                    })
                        as Box<dyn FnMut(MessageEvent)>);

                ws_obj.set_onmessage(Some(onmessage_cb.as_ref().unchecked_ref()));
                onmessage_cb.forget();

                let onerror_cb =
                    wasm_bindgen::closure::Closure::wrap(Box::new(move |e: web_sys::ErrorEvent| {
                        let _ = tx_onerror
                            .try_send(Err(TransportError::WebSocketError(format!("{:?}", e))));
                    })
                        as Box<dyn FnMut(web_sys::ErrorEvent)>);

                ws_obj.set_onerror(Some(onerror_cb.as_ref().unchecked_ref()));
                onerror_cb.forget();

                let connected_clone_open = connected_clone.clone();
                let onclose_cb =
                    wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: web_sys::CloseEvent| {
                        *connected_clone.borrow_mut() = false;
                        let _ = tx_onclose.try_send(Err(TransportError::Closed));
                    })
                        as Box<dyn FnMut(web_sys::CloseEvent)>);

                ws_obj.set_onclose(Some(onclose_cb.as_ref().unchecked_ref()));
                onclose_cb.forget();

                let onopen_cb =
                    wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: web_sys::Event| {
                        *connected_clone_open.borrow_mut() = true;
                    })
                        as Box<dyn FnMut(web_sys::Event)>);

                ws_obj.set_onopen(Some(onopen_cb.as_ref().unchecked_ref()));
                onopen_cb.forget();

                *ws_clone.borrow_mut() = Some(ws_obj);
                // Move the receiver into the struct
                let rx_inner = rx_clone;

                Ok(Self {
                    ws,
                    rx: rx_inner,
                    frame_decoder,
                    connected,
                })
            }
        }
    }

    impl SyncTransport for BrowserSyncTransport {
        type Error = TransportError;

        #[allow(refining_impl_trait)]
        fn receive(&mut self) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, Self::Error>>>> {
            let rx = self.rx.clone();
            Box::pin(async move {
                let mut rx_guard = rx.borrow_mut();
                let rx = rx_guard.as_mut().ok_or(TransportError::Closed)?;
                match rx.next().await {
                    Some(Ok(data)) => Ok(data),
                    Some(Err(e)) => Err(e),
                    None => Err(TransportError::Closed),
                }
            })
        }

        #[allow(refining_impl_trait)]
        fn send(
            &mut self,
            payload: Vec<u8>,
        ) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>>>> {
            let ws = self.ws.clone();
            Box::pin(async move {
                let ws_guard = ws.borrow();
                let ws = ws_guard.as_ref().ok_or(TransportError::Closed)?;
                let uint8_array = Uint8Array::from(&payload[..]);
                ws.send_with_u8_array(&payload[..])
                    .map_err(|e| TransportError::SendFailed(format!("{:?}", e)))
            })
        }
    }
}

/// Browser-specific SyncClient that uses the ofdb-sql SyncTransport directly.
#[cfg(feature = "browser")]
pub mod browser_client {
    use super::browser::{BrowserSyncTransport, TransportError as BrowserTransportError};
    use crate::SyncError;
    use crate::protocol::{
        AttachRequest, Frame, FrameCodec, Hello, ProtocolId, StoreIdentity, SyncMessage,
    };
    use ofdb_sql::{SessionConfig, SyncResult, SyncRole, SyncTransport};
    use std::cell::RefCell;
    use std::future::Future;
    use std::pin::Pin;
    use std::rc::Rc;
    use uuid::Uuid;

    pub type SyncDirection = crate::SyncDirection;

    /// Browser-specific token provider (no Send bound on the inner future).
    pub type BrowserTokenProvider =
        Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<String, SyncError>>>>>;

    pub struct BrowserSyncClient {
        transport: Rc<RefCell<Option<BrowserSyncTransport>>>,
        token_provider: BrowserTokenProvider,
        store_identity: StoreIdentity,
        url: String,
    }

    impl BrowserSyncClient {
        pub async fn new(
            url: &str,
            token_provider: BrowserTokenProvider,
            store_identity: StoreIdentity,
            _local_replica_id: Uuid,
        ) -> Result<Self, SyncError> {
            // We'll connect lazily on first sync
            Ok(Self {
                transport: Rc::new(RefCell::new(None)),
                token_provider,
                store_identity,
                url: url.to_string(),
            })
        }

        pub async fn sync(
            &mut self,
            direction: SyncDirection,
            database: &ofdb_sql::Database,
        ) -> Result<SyncResult, SyncError> {
            // Get fresh auth token
            let token: String = (self.token_provider)().await?;

            // Connect if not already connected
            if self.transport.borrow().is_none() {
                let transport = BrowserSyncTransport::connect(&self.url, &token)
                    .await
                    .map_err(|e| SyncError::Protocol(e.to_string()))?;
                *self.transport.borrow_mut() = Some(transport);
            }

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
                auth_token: crate::protocol::AuthToken(token.into_bytes()),
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

            // Get transport for sync (clone to get owned transport)
            let transport = self
                .transport
                .borrow()
                .as_ref()
                .cloned()
                .ok_or_else(|| SyncError::Protocol("Transport closed".to_string()))?;

            // Create a mutable reference for the sync
            let transport_ref = Rc::new(RefCell::new(Some(transport)));

            // Wrapper that implements SyncTransport by delegating to the inner transport
            struct TransportWrapper {
                inner: Rc<RefCell<Option<BrowserSyncTransport>>>,
            }

            impl SyncTransport for TransportWrapper {
                type Error = BrowserTransportError;

                #[allow(refining_impl_trait)]
                fn receive(
                    &mut self,
                ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, Self::Error>>>> {
                    let inner = self.inner.clone();
                    Box::pin(async move {
                        let mut guard = inner.borrow_mut();
                        let transport = guard.as_mut().ok_or(BrowserTransportError::Closed)?;
                        transport.receive().await
                    })
                }

                #[allow(refining_impl_trait)]
                fn send(
                    &mut self,
                    payload: Vec<u8>,
                ) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>>>> {
                    let inner = self.inner.clone();
                    Box::pin(async move {
                        let mut guard = inner.borrow_mut();
                        let transport = guard.as_mut().ok_or(BrowserTransportError::Closed)?;
                        transport.send(payload).await
                    })
                }
            }

            let mut ws_transport = TransportWrapper {
                inner: transport_ref,
            };

            // Configure sync session
            let mut config = SessionConfig::default();
            config.direction = direction.into();

            // Run sync session as Initiator
            let result = database
                .synchronize_checked(&mut ws_transport, &config, SyncRole::Initiator, || async {
                    Ok(())
                })
                .await
                .map_err(|e| SyncError::OfdbSync(e.to_string()))?;

            Ok(result)
        }

        async fn send_message(&self, msg: SyncMessage) -> Result<(), SyncError> {
            // Clone transport to get owned value (Ref is dropped after this statement)
            let mut transport = self
                .transport
                .borrow()
                .as_ref()
                .cloned()
                .ok_or_else(|| SyncError::Protocol("Transport closed".to_string()))?;
            let encoded = msg
                .encode()
                .map_err(|e| SyncError::Protocol(e.to_string()))?;
            let frame = Frame::new(0, encoded, false);
            let frame_bytes =
                FrameCodec::encode(&frame).map_err(|e| SyncError::Protocol(e.to_string()))?;
            transport
                .send(frame_bytes)
                .await
                .map_err(|e| SyncError::Protocol(e.to_string()))?;
            Ok(())
        }

        async fn receive_message(&self) -> Result<SyncMessage, SyncError> {
            // Clone transport to get owned value
            let mut transport = self
                .transport
                .borrow()
                .as_ref()
                .cloned()
                .ok_or_else(|| SyncError::Protocol("Transport closed".to_string()))?;
            let frame_bytes = transport
                .receive()
                .await
                .map_err(|e| SyncError::Protocol(e.to_string()))?;
            let frame =
                FrameCodec::decode(&frame_bytes).map_err(|e| SyncError::Protocol(e.to_string()))?;
            let msg = SyncMessage::decode(&frame.payload)
                .map_err(|e| SyncError::Protocol(e.to_string()))?;
            Ok(msg)
        }
    }
}

// Re-export for convenience
#[cfg(feature = "browser")]
pub use browser_client::{BrowserSyncClient, BrowserTokenProvider as TokenProvider, SyncDirection};
