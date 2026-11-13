#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod config;
pub mod contract;
#[cfg(feature = "replica")]
pub mod db;
pub mod device;
pub mod idp;
pub mod json_vec;
pub mod management;
#[cfg(feature = "replica")]
pub mod replica;
pub mod space_delimited;
pub mod storage;

pub mod http;

pub mod management_error;

pub mod device_error;

pub mod sync_selection;

pub mod server;

mod id;
pub use id::Id;
