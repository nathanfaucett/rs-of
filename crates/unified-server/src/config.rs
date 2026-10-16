use std::{net::SocketAddr, path::Path};

use serde::Deserialize;

#[derive(Clone)]
pub struct UnifiedConfig {
    pub listen_addr: SocketAddr,
    pub endpoint_data_dir: std::path::PathBuf,
    pub idp: idp_server::AppConfig,
    pub management: management_server::AppConfig,
    pub storage: storage_server::AppConfig,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    listen_addr: String,
    endpoint_data_dir: String,
    idp: idp_server::AppConfig,
    management: management_server::AppConfig,
    storage: storage_server::AppConfig,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:3000".to_owned(),
            endpoint_data_dir: "unified-data".to_owned(),
            idp: idp_server::AppConfig::default(),
            management: management_server::AppConfig::default(),
            storage: storage_server::AppConfig::default(),
        }
    }
}

impl UnifiedConfig {
    pub fn credential_bootstrap_directory(&self) -> std::path::PathBuf {
        self.endpoint_data_dir.with_extension("credentials")
    }

    pub fn local_provisioning_directory(&self) -> std::path::PathBuf {
        self.endpoint_data_dir.with_extension("provisioning")
    }

    pub fn idp_audience(&self) -> &str {
        self.idp
            .service_audience
            .as_deref()
            .unwrap_or(&self.idp.api_public_uri)
    }
}

impl TryFrom<&Path> for UnifiedConfig {
    type Error = config::ConfigError;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let file: FileConfig = config::Config::builder()
            .add_source(config::File::with_name(path.to_string_lossy().as_ref()))
            .add_source(
                config::Environment::with_prefix("UNIFIED")
                    .prefix_separator("__")
                    .separator("__"),
            )
            .build()?
            .try_deserialize()?;
        let listen_addr = file.listen_addr.parse().map_err(|error| {
            config::ConfigError::Message(format!("invalid listen_addr: {error}"))
        })?;
        Ok(Self {
            listen_addr,
            endpoint_data_dir: file.endpoint_data_dir.into(),
            idp: file.idp,
            management: file.management,
            storage: file.storage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::UnifiedConfig;
    use std::fs;

    #[test]
    fn bundled_config_needs_no_relationship_secrets_and_refuses_removed_configuration() {
        let root =
            std::env::temp_dir().join(format!("unified-config-{}", idp_model::model::Id::now_v7()));
        fs::create_dir(&root).expect("isolated config parent");
        let path = root.join("unified.yaml");
        fs::write(
            &path,
            "listen_addr: 127.0.0.1:3000\nendpoint_data_dir: ./data/endpoint\n",
        )
        .expect("ordinary config");
        let mut config = UnifiedConfig::try_from(path.as_path()).expect("no relationship secrets");
        assert_eq!(config.idp_audience(), config.idp.api_public_uri);
        config.idp.service_audience = Some("https://identity.example/service".into());
        assert_eq!(config.idp_audience(), "https://identity.example/service");
        assert_eq!(
            config.credential_bootstrap_directory(),
            Path::new("./data/endpoint.credentials")
        );
        for bytes in [
            "service_clients: {}\n",
            "service_clients:\n  management_idp: {}\n",
            "service_clients:\n  storage_idp: {}\n",
            "service_clients:\n  storage_management: {}\n",
            "service_clients:\n  idp_management: {}\n",
        ] {
            fs::write(&path, bytes).expect("obsolete configuration");
            assert!(UnifiedConfig::try_from(path.as_path()).is_err());
            assert_eq!(fs::read_to_string(&path).expect("unchanged config"), bytes);
        }
        fs::remove_dir_all(root).expect("isolated cleanup");
    }

    use std::path::Path;
}

#[cfg(test)]
pub(crate) fn test_config(root: &Path) -> UnifiedConfig {
    UnifiedConfig {
        listen_addr: "127.0.0.1:3000".parse().expect("test loopback"),
        endpoint_data_dir: root.join("endpoint"),
        idp: idp_server::AppConfig {
            data_dir: root.join("idp").to_string_lossy().into_owned(),
            ..Default::default()
        },
        management: management_server::AppConfig {
            data_dir: root.join("management").to_string_lossy().into_owned(),
            ..Default::default()
        },
        storage: storage_server::AppConfig {
            data_dir: root.join("storage").to_string_lossy().into_owned(),
            ..Default::default()
        },
    }
}
