use core::fmt;

use ofdb_sql::history::{SessionConfig, SyncError, SyncResult, SyncRole, SyncTransport};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_HISTORY_TRANSPORT: &str = r#"
export interface SqlHistoryTransport {
  send(data: Uint8Array): Promise<unknown>;
  receive(): Promise<Uint8Array>;
}
"#;

#[wasm_bindgen]
pub struct JsSqlHistoryTransport {
    inner: JsValue,
}

#[wasm_bindgen]
impl JsSqlHistoryTransport {
    #[allow(unused)]
    #[wasm_bindgen(constructor)]
    #[wasm_bindgen(typescript_type = "SqlHistoryTransport")]
    pub fn new(transport: JsValue) -> Self {
        Self { inner: transport }
    }
}

#[derive(Debug)]
pub struct JsSqlHistoryTransportError {
    message: String,
}

impl JsSqlHistoryTransportError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for JsSqlHistoryTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for JsSqlHistoryTransportError {}

fn js_message(value: &JsValue, fallback: &str) -> String {
    value.as_string().unwrap_or_else(|| {
        js_sys::Reflect::get(value, &JsValue::from_str("message"))
            .ok()
            .and_then(|message| message.as_string())
            .unwrap_or_else(|| fallback.to_owned())
    })
}

impl SyncTransport for JsSqlHistoryTransport {
    type Error = JsSqlHistoryTransportError;

    async fn send(&mut self, frame: Vec<u8>) -> Result<(), Self::Error> {
        let send = js_sys::Reflect::get(&self.inner, &JsValue::from_str("send"))
            .map_err(|error| {
                JsSqlHistoryTransportError::new(format!(
                    "transport send is unavailable: {}",
                    js_message(&error, "missing send method")
                ))
            })?;
        let send: js_sys::Function = send.dyn_into().map_err(|value| {
            JsSqlHistoryTransportError::new(format!(
                "transport send is not a function: {}",
                js_message(&value, "invalid send property")
            ))
        })?;
        let result = send
            .call1(&self.inner, &js_sys::Uint8Array::from(frame.as_slice()))
            .map_err(|error| {
                JsSqlHistoryTransportError::new(format!(
                    "transport send failed: {}",
                    js_message(&error, "send call rejected")
                ))
            })?;
        let promise: js_sys::Promise = result.dyn_into().map_err(|value| {
            JsSqlHistoryTransportError::new(format!(
                "transport send did not return a Promise: {}",
                js_message(&value, "invalid send result")
            ))
        })?;
        wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|error| {
                JsSqlHistoryTransportError::new(format!(
                    "transport send failed: {}",
                    js_message(&error, "send promise rejected")
                ))
            })?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<Vec<u8>, Self::Error> {
        let receive = js_sys::Reflect::get(&self.inner, &JsValue::from_str("receive"))
            .map_err(|error| {
                JsSqlHistoryTransportError::new(format!(
                    "transport receive is unavailable: {}",
                    js_message(&error, "missing receive method")
                ))
            })?;
        let receive: js_sys::Function = receive.dyn_into().map_err(|value| {
            JsSqlHistoryTransportError::new(format!(
                "transport receive is not a function: {}",
                js_message(&value, "invalid receive property")
            ))
        })?;
        let result = receive.call0(&self.inner).map_err(|error| {
            JsSqlHistoryTransportError::new(format!(
                "transport receive failed: {}",
                js_message(&error, "receive call rejected")
            ))
        })?;
        let promise: js_sys::Promise = result.dyn_into().map_err(|value| {
            JsSqlHistoryTransportError::new(format!(
                "transport receive did not return a Promise: {}",
                js_message(&value, "invalid receive result")
            ))
        })?;
        let result = wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map_err(|error| {
                JsSqlHistoryTransportError::new(format!(
                    "transport receive failed: {}",
                    js_message(&error, "receive promise rejected")
                ))
            })?;
        let bytes = js_sys::Uint8Array::from(result);
        let mut frame = vec![0; bytes.length() as usize];
        bytes.copy_to(&mut frame);
        Ok(frame)
    }
}

#[wasm_bindgen]
pub struct SqlHistoryConfig {
    inner: SessionConfig,
}

#[wasm_bindgen]
impl SqlHistoryConfig {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: SessionConfig::default(),
        }
    }

    #[wasm_bindgen(getter, js_name = "maxUnitsPerFrame")]
    pub fn max_units_per_frame(&self) -> usize {
        self.inner.max_units_per_frame
    }

    #[wasm_bindgen(setter, js_name = "maxUnitsPerFrame")]
    pub fn set_max_units_per_frame(&mut self, value: usize) {
        self.inner.max_units_per_frame = value;
    }

    #[wasm_bindgen(getter, js_name = "maxSessionBytes")]
    pub fn max_session_bytes(&self) -> usize {
        self.inner.max_session_bytes
    }

    #[wasm_bindgen(setter, js_name = "maxSessionBytes")]
    pub fn set_max_session_bytes(&mut self, value: usize) {
        self.inner.max_session_bytes = value;
    }
}

impl Default for SqlHistoryConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl From<SqlHistoryConfig> for SessionConfig {
    fn from(config: SqlHistoryConfig) -> Self {
        config.inner
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
    inner: SyncError<JsSqlHistoryTransportError>,
}

#[wasm_bindgen]
impl JsSqlHistoryError {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.inner.to_string()
    }
}

impl From<SyncError<JsSqlHistoryTransportError>> for JsSqlHistoryError {
    fn from(inner: SyncError<JsSqlHistoryTransportError>) -> Self {
        Self { inner }
    }
}
