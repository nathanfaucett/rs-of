use core::fmt::Debug;

use crate::protocol::{Message, SessionId};

/// Bidirectional transport for synchronization.
///
/// Implementations handle the underlying I/O (TCP, WebSocket, Iroh, etc.)
/// and provide async send/receive operations for Message frames.
pub trait SyncTransport {
    /// Transport-specific error type.
    type Error: Debug + Send + Sync + 'static;

    /// Send a sync message.
    ///
    /// The transport is responsible for framing and serialization.
    /// Returns an error if the send fails.
    #[cfg(not(target_arch = "wasm32"))]
    fn send(&mut self, msg: Message) -> impl Future<Output = Result<(), Self::Error>> + Send;
    #[cfg(target_arch = "wasm32")]
    fn send(&mut self, msg: Message) -> impl Future<Output = Result<(), Self::Error>>;

    /// Receive the next sync message.
    ///
    /// Returns an error if the connection is closed, times out, or
    /// the received frame is invalid.
    ///
    /// The future must own or share its receive state so synchronization can retain it
    /// across select branches without borrowing the transport or cancelling partial frames.
    #[cfg(not(target_arch = "wasm32"))]
    fn receive(&mut self) -> impl Future<Output = Result<Message, Self::Error>> + Send + 'static;
    #[cfg(target_arch = "wasm32")]
    fn receive(&mut self) -> impl Future<Output = Result<Message, Self::Error>> + 'static;

    /// Optional: Get the peer's session ID if available.
    fn session_id(&self) -> Option<SessionId> {
        None
    }
}

/// Mock transport for testing.
#[cfg(test)]
pub mod mock {
    use super::*;
    use crate::protocol::Config;
    use std::collections::VecDeque;
    use std::future::ready;
    use std::io::{Error, ErrorKind};
    use std::sync::{Arc, Mutex};

    /// In-memory mock transport for testing.
    #[derive(Debug, Clone)]
    pub struct MockTransport {
        inbound: Arc<Mutex<VecDeque<Message>>>,
        outbound: Arc<Mutex<VecDeque<Message>>>,
        config: Config,
        closed: Arc<Mutex<bool>>,
    }

    impl MockTransport {
        pub fn new(config: Config) -> Self {
            Self {
                inbound: Arc::new(Mutex::new(VecDeque::new())),
                outbound: Arc::new(Mutex::new(VecDeque::new())),
                config,
                closed: Arc::new(Mutex::new(false)),
            }
        }

        pub fn peer(&self) -> Self {
            Self {
                inbound: Arc::clone(&self.outbound),
                outbound: Arc::clone(&self.inbound),
                config: self.config.clone(),
                closed: Arc::clone(&self.closed),
            }
        }

        pub fn push_inbound(&self, msg: Message) {
            self.inbound.lock().unwrap().push_back(msg);
        }

        pub fn pop_outbound(&self) -> Option<Message> {
            self.outbound.lock().unwrap().pop_front()
        }

        pub fn outbound_len(&self) -> usize {
            self.outbound.lock().unwrap().len()
        }

        pub fn close(&self) {
            *self.closed.lock().unwrap() = true;
        }

        pub fn is_closed(&self) -> bool {
            *self.closed.lock().unwrap()
        }
    }

    impl SyncTransport for MockTransport {
        type Error = Error;

        fn send(&mut self, msg: Message) -> impl Future<Output = Result<(), Self::Error>> {
            let result = if *self.closed.lock().unwrap() {
                Err(Error::new(ErrorKind::BrokenPipe, "peer closed"))
            } else {
                self.outbound.lock().unwrap().push_back(msg);
                Ok(())
            };
            ready(result)
        }

        fn receive(&mut self) -> impl Future<Output = Result<Message, Self::Error>> + 'static {
            let result = if *self.closed.lock().unwrap() {
                Err(Error::new(ErrorKind::BrokenPipe, "peer closed"))
            } else {
                self.inbound
                    .lock()
                    .unwrap()
                    .pop_front()
                    .ok_or_else(|| Error::new(ErrorKind::BrokenPipe, "peer closed"))
            };
            ready(result)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::protocol::{make_hello, make_hello_ack};

        #[tokio::test]
        async fn mock_transport_send_receive() {
            let config = Config::default();
            let mut transport = MockTransport::new(config.clone());
            let mut peer = transport.peer();

            let hello = make_hello([1; 32], config);
            transport.send(hello.clone()).await.unwrap();

            let received = peer.receive().await.unwrap();
            assert_eq!(received, hello);
        }

        #[tokio::test]
        async fn mock_transport_bidirectional() {
            let config = Config::default();
            let mut transport = MockTransport::new(config.clone());
            let mut peer = transport.peer();

            // Client sends hello
            let hello = make_hello([1; 32], config.clone());
            transport.send(hello.clone()).await.unwrap();

            // Server receives hello
            let received = peer.receive().await.unwrap();
            assert_eq!(received, hello);

            // Server sends hello ack
            let ack = make_hello_ack(true);
            peer.send(ack.clone()).await.unwrap();

            // Client receives ack
            let received = transport.receive().await.unwrap();
            assert_eq!(received, ack);
        }
    }
}
