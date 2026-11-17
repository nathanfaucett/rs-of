#![forbid(unsafe_code)]

mod hooks;
mod mesh;
mod pairing;
mod server;
mod store;

pub use hooks::{AllowlistHook, AlpnAllowlist};
pub use mesh::{MAX_MESH_PEERS, MESH_ALPN, Message, endpoint_relay_urls};
pub use pairing::{MAX_PAIRING_PAYLOAD_LENGTH, PairingEvent, PairingOffer, validate_payload};
pub use server::{DATA_ALPN, PAIRING_ALPN, Server};
pub use store::EndpointIdStore;
