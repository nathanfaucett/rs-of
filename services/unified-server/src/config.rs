use std::{net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct UnifiedConfig {
    pub listen_addr: SocketAddr,
    pub public_base_url: Option<String>,
    pub data_dir: PathBuf,
    pub origin: String,
    pub password_manager_origin: Option<String>,
    pub join_origin: Option<String>,
    pub seed: Option<SeedConfig>,
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
