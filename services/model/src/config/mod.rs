#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod environment;
pub mod server;

pub use environment::Environment;
pub use server::ServerConfig;
