use iroh::endpoint::Connection;
use noq::{RecvStream, SendStream};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WebStream {
    _connection: Connection,
    send: SendStream,
    recv: RecvStream,
}

impl WebStream {
    pub fn new(connection: Connection, send: SendStream, recv: RecvStream) -> Self {
        Self {
            _connection: connection,
            send,
            recv,
        }
    }
}

#[wasm_bindgen]
impl WebStream {
    pub async fn write(&mut self, payload: &[u8]) -> Result<(), JsError> {
        self.send
            .write_all(payload)
            .await
            .map_err(|error| JsError::new(&error.to_string()))
    }

    pub fn finish(&mut self) -> Result<(), JsError> {
        self.send
            .finish()
            .map_err(|error| JsError::new(&error.to_string()))
    }

    pub async fn read_to_end(&mut self, limit: usize) -> Result<Vec<u8>, JsError> {
        self.recv
            .read_to_end(limit)
            .await
            .map_err(|error| JsError::new(&error.to_string()))
    }
}
