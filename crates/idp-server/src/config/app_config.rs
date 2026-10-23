use std::{io, path::Path};

use api::{Environment, ServerConfig};
use idp_service::{PasswordConfig, oauth2::OAuth2Config};
use serde::{Deserialize, Serialize};

use super::{ManagementPermissionConfig, PairingConfig};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub data_dir: String,
    pub identity_database: Option<db::RemoteDatabaseConfig>,
    pub oauth2: OAuth2Config,
    pub password: PasswordConfig,
    pub pairing: PairingConfig,
    pub key_namespace: String,
    pub control_plane_uri: Option<String>,
    pub log_level: String,
    pub ui_public_uri: String,
    pub api_public_uri: String,
    /// Expected audience/resource for IdP service access tokens.
    /// Defaults to `api_public_uri` when omitted; configurable as `LIDP_SERVICE_AUDIENCE`.
    pub service_audience: Option<String>,
    pub management_data_dir: Option<String>,
    pub management_permissions: Option<ManagementPermissionConfig>,

    pub env: Environment,
}

impl AppConfig {
    pub fn validate_installation_settings(&self) -> io::Result<()> {
        if self.oauth2.issuer.trim().is_empty() || self.key_namespace.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Canonical Issuer and key namespace are required",
            ));
        }
        Ok(())
    }

    pub fn validate_service_audience(&self) -> io::Result<()> {
        if self
            .service_audience
            .as_deref()
            .unwrap_or(&self.api_public_uri)
            .trim()
            .is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "IdP service audience must not be empty",
            ));
        }
        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            data_dir: "data".to_string(),
            identity_database: None,
            oauth2: OAuth2Config::default(),
            password: PasswordConfig::default(),
            pairing: PairingConfig::default(),
            key_namespace: "lidp".to_string(),
            control_plane_uri: None,
            ui_public_uri: "https://lidp.localhost:1355".to_string(),
            api_public_uri: "https://idp-api.localhost:1355".to_string(),
            service_audience: None,
            management_data_dir: None,
            management_permissions: None,
            log_level: "info".to_string(),
            env: Environment::default(),
        }
    }
}

impl TryFrom<&Path> for AppConfig {
    type Error = config::ConfigError;

    fn try_from(config_path: &Path) -> Result<Self, Self::Error> {
        config::Config::builder()
            .add_source(config::File::with_name(
                config_path.to_string_lossy().as_ref(),
            ))
            .add_source(config::Environment::with_prefix("LIDP"))
            .build()?
            .try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn identity_settings_validation_preserves_errors_and_audience_fallback() {
        let mut config = AppConfig::default();
        config
            .validate_installation_settings()
            .expect("default identity settings");
        config
            .validate_service_audience()
            .expect("public URI is the default audience");
        config.api_public_uri = " ".into();
        let error = config
            .validate_service_audience()
            .expect_err("empty fallback audience");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(error.to_string(), "IdP service audience must not be empty");
        config.service_audience = Some("explicit-service".into());
        config
            .validate_service_audience()
            .expect("explicit audience takes precedence");
        config.service_audience = Some("\t".into());
        assert_eq!(
            config
                .validate_service_audience()
                .expect_err("explicit whitespace is not a fallback")
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        for (issuer, namespace) in [(" ", "namespace"), ("https://issuer.test", "\t"), ("", "")] {
            config.oauth2.issuer = issuer.into();
            config.key_namespace = namespace.into();
            let error = config
                .validate_installation_settings()
                .expect_err("identity settings are required");
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
            assert_eq!(
                error.to_string(),
                "Canonical Issuer and key namespace are required"
            );
        }
    }
}
