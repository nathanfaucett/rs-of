use std::io;

use db::SyncTransport as OldSyncTransport;
use ofdb_sync::{Config, Message, SyncTransport, decode_message, encode_message};

/// Adapter that implements the new SyncTransport (Message frames) by wrapping
/// a mutable reference to the old SyncTransport (Vec<u8> frames)
/// and using the ofdb_sync protocol codec.
pub struct SyncAdapter<'a, T> {
    inner: &'a mut T,
    config: Config,
}

impl<'a, T> SyncAdapter<'a, T> {
    pub fn new(inner: &'a mut T) -> Self {
        Self {
            inner,
            config: Config::default(),
        }
    }
}

impl<'a, T: OldSyncTransport<Error = io::Error> + Send> SyncTransport for SyncAdapter<'a, T> {
    type Error = io::Error;

    fn send(&mut self, msg: Message) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let bytes = encode_message(&msg, &self.config)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
        let config = self.config.clone();
        let inner = self.inner;

        async move {
            let bytes = bytes?;
            let len = bytes.len() as u32;
            let mut frame = Vec::with_capacity(4 + bytes.len());
            frame.extend_from_slice(&len.to_be_bytes());
            frame.extend_from_slice(&bytes);
            inner.send(frame).await
        }
    }

    fn receive(&mut self) -> impl Future<Output = Result<Message, Self::Error>> + Send {
        let fut = self.inner.receive();
        let config = self.config.clone();

        async move {
            let frame = fut.await?;
            // Decode as Message (postcard format)
            decode_message::<ofdb_sync::Error<io::Error>>(&frame, &config)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
        }
    }
}
