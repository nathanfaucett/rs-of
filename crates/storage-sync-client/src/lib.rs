//! Sync client for app-to-device mesh synchronization.
//!
//! This crate provides a native and browser-compatible client for synchronizing
//! SQL databases with the storage-server's sync WebSocket endpoint.

#[cfg(feature = "browser")]
pub mod browser;
#[cfg(feature = "native")]
pub mod native;

pub mod client;
pub mod protocol;
pub mod transport;

// SyncError is available regardless of feature
pub use client::{SyncError, WsTransportBridge};

#[cfg(feature = "native")]
pub use client::{SyncClient, TokenProvider, WsTransportBridgeShared};

#[cfg(feature = "browser")]
pub use browser::browser_client::{BrowserSyncClient, BrowserTokenProvider};

pub use protocol::*;
pub use transport::*;
