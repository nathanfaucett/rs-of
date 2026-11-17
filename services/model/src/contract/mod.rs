#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod access_token;
pub mod authorization_details;
pub mod error_response;
pub mod health_response;
pub mod health_status;
pub mod id_token;
pub mod principal_type;
pub mod refresh_token;
pub mod replication;
pub mod standard_claims;
pub mod token_response;
pub mod token_type;
pub mod token_use;
pub mod version_response;
