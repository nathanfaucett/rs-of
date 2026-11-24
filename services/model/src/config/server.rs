#[cfg(feature = "std")]
use std::env;

use core::net::{IpAddr, Ipv4Addr};

#[cfg(not(feature = "std"))]
use alloc::string::String;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
    pub gzip: bool,
    pub prefix: Option<String>,
    pub setup_ui_origin: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: default_port(),
            gzip: true,
            prefix: None,
            setup_ui_origin: None,
        }
    }
}

impl ServerConfig {
    #[must_use]
    pub fn prefix(&self) -> &str {
        self.prefix.as_deref().unwrap_or("").trim_end_matches('/')
    }
}

#[cfg(feature = "std")]
fn default_port() -> u16 {
    env::var("PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(3000)
}

#[cfg(not(feature = "std"))]
const fn default_port() -> u16 {
    3000
}
