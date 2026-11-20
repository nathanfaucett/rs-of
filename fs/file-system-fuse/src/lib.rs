//! Linux FUSE mounts for opened Full-residency File Systems.
//!
//! The adapter supports lookup, attributes, file open/read/create/write/flush/fsync/release,
//! directory creation, file unlink, empty-directory removal, and directory open/read/release.
//! It performs no network access, authorization changes, or Residency changes. Truncate, rename,
//! recursive deletion, Passthrough retrieval, and other operating systems are unsupported.

#![forbid(unsafe_code)]

#[cfg(not(target_os = "linux"))]
compile_error!("file-system-fuse supports Linux only");

mod filesystem;
mod mount;
mod path;

pub use mount::{MountError, MountGuard, mount};
