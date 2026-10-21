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
    use std::{fs, path::Path};

    use super::UnifiedConfig;

    #[test]
    fn default_unified_config_loads_without_relationship_secrets() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/unified/config.yaml");
        let config = UnifiedConfig::try_from(path.as_path()).expect("default unified config");
        assert_eq!(config.listen_addr.to_string(), "127.0.0.1:3000");
        assert_eq!(
            config.endpoint_data_dir,
            Path::new("./data/unified/endpoint")
        );
        assert_eq!(config.idp.data_dir, "./data/unified/idp");
        assert_eq!(config.management.data_dir, "./data/unified/management");
        assert_eq!(config.storage.data_dir, "./data/unified/storage");
        assert_eq!(config.idp.ui_public_uri, "https://of.localhost:1355");
        assert_eq!(
            config.idp.api_public_uri,
            "https://unified.localhost:1355/idp"
        );
        assert_eq!(
            config.management.api_public_uri,
            "https://unified.localhost:1355/management"
        );
        assert_eq!(
            config.storage.api_public_uri,
            "https://unified.localhost:1355/storage"
        );
        assert_eq!(config.idp_audience(), config.idp.api_public_uri);
        assert_eq!(
            config.management.storage_audience,
            config.storage.api_public_uri
        );
        assert!(config.idp.oauth2.issuer.is_empty());
        assert!(config.idp.oauth2.local_signer.is_none());
        assert!(config.idp.management_data_dir.is_none());
        assert!(config.idp.management_permissions.is_none());
        assert!(config.management.expected_issuer.is_empty());
        assert!(
            config
                .management
                .idp_permission_evaluator_client_id
                .is_none()
        );
        assert!(config.storage.idp_issuer_uri.is_empty());
        assert!(config.storage.idp_oauth_client_id.is_none());
        assert!(config.storage.idp_oauth_client_secret.is_none());
        assert!(config.storage.management_oauth_client_id.is_none());
        assert!(config.storage.management_oauth_client_secret.is_none());
        assert!(config.storage.management_endpoint_id.is_none());
    }

    #[test]
    fn default_standalone_configs_load_without_relationship_secrets() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config");
        let idp = idp_server::AppConfig::try_from(directory.join("idp/config.yaml").as_path())
            .expect("default IdP config");
        let management = management_server::AppConfig::try_from(
            directory.join("management/config.yaml").as_path(),
        )
        .expect("default Management config");
        let storage =
            storage_server::AppConfig::try_from(directory.join("storage/config.yaml").as_path())
                .expect("default Storage config");
        for (name, server, data_dir, public_uri, port) in [
            ("idp", &idp.server, &idp.data_dir, &idp.api_public_uri, 3000),
            (
                "management",
                &management.server,
                &management.data_dir,
                &management.api_public_uri,
                3001,
            ),
            (
                "storage",
                &storage.server,
                &storage.data_dir,
                &storage.api_public_uri,
                3002,
            ),
        ] {
            assert_eq!(server.host.to_string(), "127.0.0.1");
            assert_eq!(server.port, port);
            assert_eq!(server.prefix(), "");
            assert_eq!(
                server.setup_ui_origin.as_deref(),
                Some("https://of.localhost:1355")
            );
            assert_eq!(data_dir, &format!("./data/{name}"));
            assert_eq!(public_uri, &format!("https://{name}-api.localhost:1355"));
        }
        assert_eq!(idp.ui_public_uri, "https://of.localhost:1355");
        assert!(idp.oauth2.issuer.is_empty());
        assert!(idp.oauth2.local_signer.is_none());
        assert!(idp.management_data_dir.is_none());
        assert!(idp.management_permissions.is_none());
        assert!(management.expected_issuer.is_empty());
        assert!(management.idp_permission_evaluator_client_id.is_none());
        assert!(storage.idp_issuer_uri.is_empty());
        assert!(storage.idp_oauth_client_id.is_none());
        assert!(storage.idp_oauth_client_secret.is_none());
        assert!(storage.management_oauth_client_id.is_none());
        assert!(storage.management_oauth_client_secret.is_none());
        assert!(storage.management_endpoint_id.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn default_unified_config_supports_fresh_explicit_bootstrap_and_retained_issuer() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/unified/config.yaml");
        let mut config = UnifiedConfig::try_from(path.as_path()).expect("default unified config");
        let root = std::env::temp_dir().join(format!(
            "default-unified-bootstrap-{}",
            idp_model::model::Id::now_v7()
        ));
        config.endpoint_data_dir = root.join(&config.endpoint_data_dir);
        for data_dir in [
            &mut config.idp.data_dir,
            &mut config.management.data_dir,
            &mut config.storage.data_dir,
        ] {
            *data_dir = root.join(&*data_dir).to_string_lossy().into_owned();
        }
        let host = crate::BootstrapConfig::create_explicit_http(config.clone())
            .expect("fresh default explicit bootstrap");
        assert!(host.purpose() == crate::BootstrapPurpose::AwaitingChoice);
        assert!(host.config().idp.oauth2.issuer.is_empty());
        let setup = crate::SetupHost::create_http(host).expect("default proxy setup host");
        assert!(setup.admission_path().is_file());
        drop(setup);
        let mut host = crate::BootstrapConfig::open_explicit(config.clone())
            .expect("reopen default neutral host");
        assert!(host.config().idp.oauth2.issuer.is_empty());
        host.select_create().expect("app chooses Create");
        let issuer = host.config().idp.oauth2.issuer;
        assert!(!issuer.is_empty());
        assert_ne!(issuer, config.idp.api_public_uri);
        drop(host);
        let host = crate::BootstrapConfig::open_explicit(config.clone())
            .expect("reopen selected installation");
        assert_eq!(host.config().idp.oauth2.issuer, issuer);
        assert!(host.purpose() == crate::BootstrapPurpose::Create);
        for directory in [
            &config.endpoint_data_dir,
            Path::new(&config.idp.data_dir),
            Path::new(&config.management.data_dir),
            Path::new(&config.storage.data_dir),
        ] {
            assert!(
                !directory.exists(),
                "bootstrap must not create domain state"
            );
        }
        drop(host);
        fs::remove_dir_all(root).expect("isolated bootstrap cleanup");
    }

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
