use std::{
    io,
    path::{Path, PathBuf},
};

use api::ServerConfig;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub data_dir: String,
    pub identity_database: Option<db::RemoteDatabaseConfig>,
    pub identity_socket: Option<PathBuf>,
    pub identity_socket_deadline_ms: u64,

    pub log_level: String,
    pub api_public_uri: String,
    pub idp_api_base: String,
    pub storage_api_base: String,
    pub expected_issuer: String,
    pub storage_audience: String,
    pub idp_permission_evaluator_client_id: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            data_dir: "data".to_string(),
            identity_database: None,
            identity_socket: None,
            identity_socket_deadline_ms: 10_000,

            log_level: "info".to_string(),
            api_public_uri: "https://management-api.localhost:1355".to_string(),
            idp_api_base: String::new(),
            storage_api_base: String::new(),
            expected_issuer: String::new(),
            storage_audience: String::new(),
            idp_permission_evaluator_client_id: None,
        }
    }
}

impl<'a> TryFrom<&'a Path> for AppConfig {
    type Error = config::ConfigError;

    fn try_from(config_path: &'a Path) -> Result<Self, Self::Error> {
        config::Config::builder()
            .add_source(config::File::with_name(
                config_path.to_string_lossy().as_ref(),
            ))
            .add_source(config::Environment::with_prefix("SERVER"))
            .build()?
            .try_deserialize()
    }
}

impl AppConfig {
    pub(crate) fn validate_identity_connection(&self) -> io::Result<()> {
        match (&self.identity_database, &self.identity_socket) {
            (Some(remote), None) => remote.validate(),
            (None, Some(path))
                if path.is_absolute()
                    && (1..=300_000).contains(&self.identity_socket_deadline_ms) =>
            {
                #[cfg(unix)]
                {
                    Ok(())
                }
                #[cfg(not(unix))]
                {
                    Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "identity_socket requires Unix",
                    ))
                }
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "select exactly one of identity_database or identity_socket; identity_socket requires an absolute path and a deadline of 1..=300000 ms",
            )),
        }
    }
}
