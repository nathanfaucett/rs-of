use ofdb_sql::history::{MAX_MESSAGE_BYTES, SyncError, SyncResult, SyncRole, SyncTransport};
use wasm_bindgen::prelude::*;

use crate::sync::{JsPushSyncTransportError, JsSqlSyncTransport, receive_bytes, send_bytes};

impl SyncTransport for JsSqlSyncTransport {
    type Error = JsPushSyncTransportError;

    async fn send(&mut self, frame: Vec<u8>) -> Result<(), Self::Error> {
        send_bytes(self.js_value(), &frame, "transport send").await
    }

    async fn receive(&mut self) -> Result<Vec<u8>, Self::Error> {
        receive_bytes(self.js_value(), Some(MAX_MESSAGE_BYTES)).await
    }
}

#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqlHistoryRole {
    Initiator = 0,
    Responder = 1,
}

impl From<SqlHistoryRole> for SyncRole {
    fn from(role: SqlHistoryRole) -> Self {
        match role {
            SqlHistoryRole::Initiator => Self::Initiator,
            SqlHistoryRole::Responder => Self::Responder,
        }
    }
}

#[wasm_bindgen]
#[derive(Clone, Debug, Default)]
pub struct SqlHistoryResult {
    inner: SyncResult,
}

#[wasm_bindgen]
impl SqlHistoryResult {
    #[wasm_bindgen(getter, js_name = "receivedUnits")]
    pub fn received_units(&self) -> usize {
        self.inner.received_units
    }

    #[wasm_bindgen(getter, js_name = "sentUnits")]
    pub fn sent_units(&self) -> usize {
        self.inner.sent_units
    }

    #[wasm_bindgen(getter, js_name = "receivedChanges")]
    pub fn received_changes(&self) -> usize {
        self.inner.received_changes
    }

    #[wasm_bindgen(getter, js_name = "sentChanges")]
    pub fn sent_changes(&self) -> usize {
        self.inner.sent_changes
    }

    #[wasm_bindgen(getter, js_name = "receivedSnapshots")]
    pub fn received_snapshots(&self) -> usize {
        self.inner.received_snapshots
    }

    #[wasm_bindgen(getter, js_name = "sentSnapshots")]
    pub fn sent_snapshots(&self) -> usize {
        self.inner.sent_snapshots
    }
}

impl From<SyncResult> for SqlHistoryResult {
    fn from(inner: SyncResult) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
pub struct JsSqlHistoryError {
    inner: SyncError<JsPushSyncTransportError>,
}

#[wasm_bindgen]
impl JsSqlHistoryError {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.inner.to_string()
    }
}

impl From<SyncError<JsPushSyncTransportError>> for JsSqlHistoryError {
    fn from(inner: SyncError<JsPushSyncTransportError>) -> Self {
        Self { inner }
    }
}
