#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod device;
pub mod idp;
pub mod management;
pub mod storage;
pub mod storage_api;
