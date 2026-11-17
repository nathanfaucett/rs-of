use std::{net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct UnifiedConfig {
    pub listen_addr: SocketAddr,
    pub data_dir: PathBuf,
    pub origin: String,
    pub tls: TlsConfig,
    pub seed: Option<SeedConfig>,
}

#[derive(Clone, Debug)]
pub enum TlsConfig {
    AutoGenerate,
    Disabled,
    External {
        cert_path: PathBuf,
        key_path: PathBuf,
    },
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedConfig {
    pub users: Vec<SeedUser>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeedUser {
    pub username: String,
    pub password: String,
}
