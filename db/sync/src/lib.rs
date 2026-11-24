#![allow(dead_code, reason = "public API crate - items exported for consumers")]

mod protocol;
mod subscription;
mod transport;

#[cfg(all(feature = "iroh", not(target_arch = "wasm32")))]
mod iroh_transport;

pub use protocol::{
    AckConfig, Config, Error, HelloConfig, MAX_MESSAGE_BYTES, Message, SyncResult, SyncRole,
    decode_message, encode_message,
};
pub use transport::SyncTransport;

#[cfg(all(feature = "std", not(all(feature = "wasm", target_arch = "wasm32"))))]
mod sync;

#[cfg(all(feature = "std", not(all(feature = "wasm", target_arch = "wasm32"))))]
pub use sync::{SynchronizeConfig, SynchronizeError, synchronize};

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
mod wasm_sync;

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
pub use wasm_sync::{
    PushConfig, PushMessage, PushSyncTransport, SynchronizeConfig, SynchronizeError, synchronize,
};

pub use protocol::{PushApplyResult, PushResult, PushSynchronizeConfig, PushSynchronizeError};

pub use subscription::ChangeSubscription;

pub use btree::ChangeEvent;

#[cfg(all(feature = "iroh", not(target_arch = "wasm32")))]
pub use iroh_transport::IrohTransport;
