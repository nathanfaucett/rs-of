use std::io::{self, ErrorKind};

use iroh::endpoint::{RecvStream, SendStream};

pub use ofdb_sql::history::SyncTransport;

const MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct DatabaseTransport {
    send: SendStream,
    recv: RecvStream,
}

impl DatabaseTransport {
    #[must_use]
    pub const fn new(send: SendStream, recv: RecvStream) -> Self {
        Self { send, recv }
    }
}

impl SyncTransport for DatabaseTransport {
    type Error = io::Error;

    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        let mut prefix = [0; 4];
        self.recv
            .read_exact(&mut prefix)
            .await
            .map_err(io::Error::other)?;
        let length = u32::from_be_bytes(prefix) as usize;
        if length > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "database sync frame too large",
            ));
        }
        let mut frame = vec![0; length];
        self.recv
            .read_exact(&mut frame)
            .await
            .map_err(io::Error::other)?;
        Ok(frame)
    }

    async fn send(&mut self, frame: Vec<u8>) -> io::Result<()> {
        if frame.len() > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "database sync frame too large",
            ));
        }
        let length = u32::try_from(frame.len()).expect("frame length is bounded to 1 MiB");
        self.send
            .write_all(&length.to_be_bytes())
            .await
            .map_err(io::Error::other)?;
        self.send.write_all(&frame).await.map_err(io::Error::other)
    }
}
