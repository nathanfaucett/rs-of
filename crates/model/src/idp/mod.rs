#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod contract;
pub mod model;
#[cfg(feature = "ofdb")]
pub mod ofdb;
#[cfg(feature = "replica")]
pub use ofdb as replica;
