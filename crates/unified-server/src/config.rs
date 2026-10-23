use std::{
    io,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use api::LocalTls;
use idp_server::local_provisioning::{absolute, require_absent};

use serde::Deserialize;

#[derive(Clone)]
pub struct UnifiedConfig {
    pub listen_addr: SocketAddr,
    pub endpoint_data_dir: std::path::PathBuf,
    pub idp: idp_server::AppConfig,
    pub management: management_server::AppConfig,
    pub device: device_server::AppConfig,
    pub storage: storage_server::AppConfig,
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

impl api::LocalTlsConfig for UnifiedConfig {
    fn tls_directory(&self) -> std::path::PathBuf {
        self.endpoint_data_dir.with_extension("tls")
    }
    fn tls_address(&self) -> SocketAddr {
        self.listen_addr
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    listen_addr: String,
    endpoint_data_dir: String,
    idp: idp_server::AppConfig,
    management: management_server::AppConfig,
    device: device_server::AppConfig,
    storage: storage_server::AppConfig,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:3000".to_owned(),
            endpoint_data_dir: "unified-data".to_owned(),
            idp: idp_server::AppConfig::default(),
            management: management_server::AppConfig::default(),
            device: device_server::AppConfig::default(),
            storage: storage_server::AppConfig::default(),
        }
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
            device: file.device,
            storage: file.storage,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::UnifiedConfig;

    #[cfg(unix)]
    #[test]
    fn explicit_bootstrap_retains_concrete_server_options_and_public_references() {
        let root = std::env::temp_dir().join(format!(
            "unified-concrete-settings-{}",
            idp_model::model::Id::now_v7()
        ));
        fs::create_dir(&root).expect("isolated config parent");
        let path = root.join("config.yaml");
        fs::write(&path, "management:\n  log_level: debug\n  idp_api_base: https://identity.test/idp\n  storage_api_base: https://resources.test/storage\n  storage_audience: https://resources.test/audience\n  server:\n    port: 4321\nstorage:\n  log_level: trace\n  idp_api_base_uri: https://identity.test/idp\n  management_api_base_uri: https://control.test/management\n  management_service_audience: https://control.test/audience\n  server:\n    port: 5432\n").expect("nondefault server configuration");
        let mut config = UnifiedConfig::try_from(path.as_path()).expect("concrete YAML settings");
        config.endpoint_data_dir = root.join("endpoint");
        config.idp.data_dir = root.join("idp").to_string_lossy().into_owned();
        config.device.data_dir = root.join("management").to_string_lossy().into_owned();
        config.storage.data_dir = root.join("storage").to_string_lossy().into_owned();
        let host = crate::BootstrapConfig::create_explicit_http(config.clone())
            .expect("explicit neutral host");
        let setup = crate::SetupHost::create_http(host).expect("neutral host prerequisites");
        drop(setup);
        let mut host = crate::BootstrapConfig::open_explicit(config.clone())
            .expect("reopen exact configuration");
        host.select_create().expect("select Create");
        let selected = host.config();
        assert!(!selected.idp.oauth2.issuer.is_empty());
        let bootstrap = crate::CredentialBootstrap::create(selected.clone())
            .expect("retain public bootstrap references");
        let loaded = bootstrap
            .load()
            .expect("normalize without losing concrete options");
        assert_eq!(loaded.management.log_level, "debug");
        assert_eq!(loaded.management.server.port, 4321);
        assert_eq!(loaded.management.idp_api_base, "https://identity.test/idp");
        assert_eq!(
            loaded.management.storage_api_base,
            "https://resources.test/storage"
        );
        assert_eq!(
            loaded.management.storage_audience,
            "https://resources.test/audience"
        );
        assert_eq!(loaded.storage.log_level, "trace");
        assert_eq!(loaded.storage.server.port, 5432);
        assert_eq!(loaded.storage.idp_api_base_uri, "https://identity.test/idp");
        assert_eq!(
            loaded.storage.device_api_base_uri,
            "https://control.test/management"
        );
        assert_eq!(
            loaded.storage.device_service_audience.as_deref(),
            Some("https://control.test/audience")
        );
        assert_eq!(loaded.idp.oauth2.issuer, selected.idp.oauth2.issuer);
        let references: serde_json::Value = serde_json::from_slice(
            &fs::read(
                loaded
                    .credential_bootstrap_directory()
                    .join("credentials.json"),
            )
            .expect("retained journal"),
        )
        .expect("public journal JSON");
        assert_eq!(
            references["storage_audience"],
            "https://resources.test/audience"
        );
        drop(bootstrap);
        let mut conflicting = selected;
        conflicting.management.storage_audience = "https://different.test".into();
        assert!(crate::CredentialBootstrap::open(conflicting).is_err());
        drop(host);
        fs::remove_dir_all(root).expect("isolated cleanup");
    }

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
        assert_eq!(config.device.data_dir, "./data/unified/management");
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
            config.device.storage_audience,
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
        assert!(config.storage.device_oauth_client_id.is_none());
        assert!(config.storage.device_oauth_client_secret.is_none());
        assert!(config.storage.device_endpoint_id.is_none());
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
        assert!(storage.device_oauth_client_id.is_none());
        assert!(storage.device_oauth_client_secret.is_none());
        assert!(storage.device_endpoint_id.is_none());
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
        let settings = &mut config;
        for data_dir in [
            &mut settings.idp.data_dir,
            &mut settings.device.data_dir,
            &mut settings.storage.data_dir,
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
        let issuer = host.config().idp.oauth2.issuer.clone();
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
            Path::new(&config.device.data_dir),
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
        device: device_server::AppConfig {
            data_dir: root.join("device").to_string_lossy().into_owned(),
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

fn roots(config: &UnifiedConfig) -> io::Result<[PathBuf; 7]> {
    Ok([
        absolute(Path::new(&config.idp.data_dir))?,
        absolute(Path::new(&config.device.data_dir))?,
        absolute(Path::new(&config.storage.data_dir))?,
        absolute(&config.endpoint_data_dir)?,
        absolute(&config.credential_bootstrap_directory())?,
        absolute(&config.local_provisioning_directory())?,
        absolute(&LocalTls::directory(config))?,
    ])
}

pub fn require_host_separation(config: &UnifiedConfig, host: &Path) -> io::Result<()> {
    let host = absolute(host)?;
    for root in roots(config)? {
        if host.starts_with(&root)
            || [
                "setup-admission",
                "host.json",
                "host.pending",
                "join-admission.json",
                "join-admission.pending",
                "join-credentials.attempted",
                "join-credentials.pending",
                "join-owner.attempted",
                "join-owner.pending",
                "operation.lock",
            ]
            .iter()
            .any(|reserved| root.starts_with(host.join(reserved)))
        {
            return Err(io::Error::other(
                "host metadata/admission must be separate from domain and prerequisite roots",
            ));
        }
    }
    Ok(())
}

pub fn require_fresh_roots(
    config: &UnifiedConfig,
    tls_absent: bool,
    https: bool,
) -> io::Result<()> {
    if config.idp.oauth2.local_signer.is_some() {
        return Err(io::Error::other(
            "neutral startup requires no configured Node signer",
        ));
    }
    LocalTls::api_base(config.listen_addr)?;
    #[cfg(unix)]
    if https {
        api::setup_origin(&config.idp.ui_public_uri)?;
    } else {
        idp_server::hosted_setup::setup_origin(&config.idp.ui_public_uri, false)?;
    }
    let roots = roots(config)?;
    for (index, root) in roots.iter().enumerate() {
        if index != roots.len() - 1 || tls_absent {
            require_absent(root)?;
        }
        if roots[..index]
            .iter()
            .any(|other| root.starts_with(other) || other.starts_with(root))
        {
            return Err(io::Error::other("setup and context roots must not overlap"));
        }
    }
    Ok(())
}

pub fn validate_config(config: &UnifiedConfig) -> io::Result<()> {
    if config.idp_audience().trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "IdP service audience must not be empty",
        ));
    }
    Ok(())
}
