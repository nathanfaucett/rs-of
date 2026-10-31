use std::{
    io::{self, Write},
    net::SocketAddr,
    path::Path,
};

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
    pub fn local_dynamic(root: &Path) -> io::Result<Self> {
        let path = root.join("listen.addr");
        let address = if api::setup_files::present(&path)? {
            String::from_utf8(api::setup_files::read_private(&path)?)
                .map_err(io::Error::other)?
                .parse()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        } else {
            if api::setup_files::present(root)? && std::fs::read_dir(root)?.next().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported retained Unified Host layout; preserve the root and use the previous binary for recovery/reset, or choose a new empty directory",
                ));
            }
            api::setup_files::create_private_directory(root, true)?;
            let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
            let address = listener.local_addr()?;
            let mut file = api::setup_files::private_options(true).open(path)?;
            write!(file, "{address}")?;
            file.sync_all()?;
            api::setup_files::sync_ancestry(root)?;
            address
        };
        Ok(Self::local(root, address))
    }

    pub fn local(root: &Path, listen_addr: SocketAddr) -> Self {
        let base = format!("https://{listen_addr}");
        let mut idp = idp_server::AppConfig {
            data_dir: root.join("idp").to_string_lossy().into_owned(),
            server: api::ServerConfig {
                prefix: Some("/idp".into()),
                ..Default::default()
            },
            api_public_uri: format!("{base}/idp"),
            ..Default::default()
        };
        idp.oauth2.issuer = idp.api_public_uri.clone();
        let device = device_server::AppConfig {
            data_dir: root.join("device").to_string_lossy().into_owned(),
            server: api::ServerConfig {
                prefix: Some("/device".into()),
                ..Default::default()
            },
            api_public_uri: format!("{base}/device"),
            ..Default::default()
        };
        let storage = storage_server::AppConfig {
            data_dir: root.join("storage").to_string_lossy().into_owned(),
            server: api::ServerConfig {
                prefix: Some("/storage".into()),
                ..Default::default()
            },
            api_public_uri: format!("{base}/storage"),
            ..Default::default()
        };
        let management = management_server::AppConfig {
            data_dir: root.join("management").to_string_lossy().into_owned(),
            server: api::ServerConfig {
                prefix: Some("/management".into()),
                ..Default::default()
            },
            api_public_uri: format!("{base}/management"),
            ..Default::default()
        };
        Self {
            listen_addr,
            endpoint_data_dir: root.join("endpoint"),
            idp,
            management,
            device,
            storage,
        }
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
