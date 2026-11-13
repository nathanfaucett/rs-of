use std::{env, net::SocketAddr};

/// Get the bind address using PORT env var (set by portless or manually), defaults to 127.0.0.1:3000.
pub fn bind_addr() -> SocketAddr {
    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);
    SocketAddr::from(([127, 0, 0, 1], port))
}

/// Get the public base URL from env var (set by portless via PORTLESS_URL or manually).
/// Returns None if not set.
pub fn public_base_url() -> Option<String> {
    env::var("PUBLIC_BASE_URL")
        .ok()
        .or_else(|| env::var("PORTLESS_URL").ok())
}
