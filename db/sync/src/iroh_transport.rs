use std::{
    io::{Error, ErrorKind},
    sync::Arc,
};

use iroh::endpoint::{RecvStream, SendStream};
use tokio::sync::Mutex;

use crate::protocol::{
    Config, Error as ProtocolError, MAX_MESSAGE_BYTES, Message, decode_message, encode_message,
};
use crate::transport::SyncTransport;

/// Length-prefixed sync frames over an already-authorized Iroh bidirectional stream.
/// Each frame is a big-endian u32 byte length followed by exactly that many bytes.
#[derive(Debug)]
pub struct IrohTransport {
    send: SendStream,
    recv: Arc<Mutex<RecvStream>>,
}

impl IrohTransport {
    #[must_use]
    pub fn new(send: SendStream, recv: RecvStream) -> Self {
        Self {
            send,
            recv: Arc::new(Mutex::new(recv)),
        }
    }
}

impl SyncTransport for IrohTransport {
    type Error = Error;

    fn send(&mut self, msg: Message) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let config = Config::default();
        let bytes =
            encode_message(&msg, &config).map_err(|e| Error::new(ErrorKind::InvalidData, e));
        let send = &mut self.send;
        async move {
            let bytes = bytes?;
            if bytes.len() > MAX_MESSAGE_BYTES {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "sync frame exceeds 1 MiB",
                ));
            }
            let length = u32::try_from(bytes.len()).expect("bounded sync frame fits u32");
            send.write_all(&length.to_be_bytes())
                .await
                .map_err(Error::other)?;
            send.write_all(&bytes).await.map_err(Error::other)
        }
    }

    fn receive(&mut self) -> impl Future<Output = Result<Message, Self::Error>> + Send + 'static {
        let config = Config::default();
        let recv = Arc::clone(&self.recv);
        async move {
            let mut recv = recv.lock().await;
            let mut prefix = [0; 4];
            recv.read_exact(&mut prefix).await.map_err(Error::other)?;
            let length = u32::from_be_bytes(prefix) as usize;
            if length > MAX_MESSAGE_BYTES {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "sync frame exceeds 1 MiB",
                ));
            }
            let mut frame = vec![0; length];
            recv.read_exact(&mut frame).await.map_err(Error::other)?;
            drop(recv);
            decode_message::<ProtocolError<Error>>(&frame, &config)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))
        }
    }
}
