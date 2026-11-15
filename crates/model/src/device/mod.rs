#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod contract;
pub mod device;
pub mod device_state_sql;
pub mod selection_policy;
