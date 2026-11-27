#![forbid(unsafe_code)]

mod kv;
mod sql;
#[cfg(feature = "sync")]
mod sql_history;
mod sql_migration;
mod sync;
mod value;

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub async fn init() {
    console_error_panic_hook::set_once();
}

pub use kv::{KvClient, KvDatabase, KvTransaction};
pub use sql::{SqlClient, SqlCommitWatch, SqlDatabase, SqlTransaction};
#[cfg(feature = "sync")]
pub use sql_history::{JsSqlHistoryError, SqlHistoryResult, SqlHistoryRole};
#[cfg(feature = "sync")]
pub use sync::{
    JsChangeEvent, JsKvSyncError, JsKvSyncErrorKind, JsKvSyncTransport, JsPushSyncError,
    JsPushSyncErrorKind, JsPushSyncTransport, JsPushSyncTransportError, JsSqlSyncError,
    JsSqlSyncErrorKind, JsSqlSyncTransport, KvChangeSubscription, PushConfig,
    PushSynchronizeConfig, PushSynchronizeResult, SqlChangeSubscription,
};
