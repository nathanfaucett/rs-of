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

// SyncError and SyncDirection are available regardless of feature
pub use client::{SyncDirection, SyncError};

#[cfg(feature = "native")]
pub use client::{SyncClient, TokenProvider};

#[cfg(feature = "browser")]
pub use browser::browser_client::{BrowserSyncClient, BrowserTokenProvider};

pub use protocol::*;
