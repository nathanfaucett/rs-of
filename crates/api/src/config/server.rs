use std::net::{IpAddr, Ipv4Addr};

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
            host: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            port: std::env::var("PORT")
                .ok()
                .and_then(|port| port.parse::<u16>().ok())
                .unwrap_or(3000),
            gzip: true,
            prefix: None,
            setup_ui_origin: None,
        }
    }
}

impl ServerConfig {
    pub fn prefix(&self) -> &str {
        if let Some(prefix) = self.prefix.as_ref() {
            prefix
        } else {
            ""
        }
        .trim_end_matches("/")
    }
}
