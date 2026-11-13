use std::{
    io::{self, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
};

use server_common::{LocalTlsConfig, ServerConfig, setup_files};

pub struct DesktopConfig {
    pub listen_addr: SocketAddr,
    pub endpoint_data_dir: PathBuf,
    pub idp: idp_server::AppConfig,
    pub management: management_server::AppConfig,
    pub device: device_server::AppConfig,
    pub storage: storage_server::AppConfig,
}

impl DesktopConfig {
    pub fn open(root: &Path) -> io::Result<Self> {
        let path = root.join("listen.addr");
        let listen_addr = if setup_files::present(&path)? {
            String::from_utf8(setup_files::read_private(&path)?)
                .map_err(io::Error::other)?
                .parse()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        } else {
            if setup_files::present(root)? && std::fs::read_dir(root)?.next().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained desktop state has no listen address; restore exact state or explicitly reset",
                ));
            }
            setup_files::create_private_directory(root, true)?;
            let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
            let address = listener.local_addr()?;
            let mut file = setup_files::private_options(true).open(path)?;
            write!(file, "{address}")?;
            file.sync_all()?;
            setup_files::sync_ancestry(root)?;
            address
        };
        let base = format!("https://{listen_addr}");
        let origin = crate::native_host::native_origin()
            .to_str()
            .map_err(io::Error::other)?
            .to_owned();
        let mut idp = idp_server::AppConfig {
            data_dir: root.join("idp").to_string_lossy().into_owned(),
            server: ServerConfig {
                prefix: Some("/idp".into()),
                ..Default::default()
            },
            api_public_uri: format!("{base}/idp"),
            ui_public_uri: origin.clone(),
            ..Default::default()
        };
        idp.oauth2.issuer = idp.api_public_uri.clone();
        Ok(Self {
            listen_addr,
            endpoint_data_dir: root.join("endpoint"),
            idp,
            device: device_server::AppConfig {
                data_dir: root.join("device").to_string_lossy().into_owned(),
                server: ServerConfig {
                    prefix: Some("/device".into()),
                    setup_ui_origin: Some(origin.clone()),
                    ..Default::default()
                },
                api_public_uri: format!("{base}/device"),
                ..Default::default()
            },
            storage: storage_server::AppConfig {
                data_dir: root.join("storage").to_string_lossy().into_owned(),
                server: ServerConfig {
                    prefix: Some("/storage".into()),
                    setup_ui_origin: Some(origin),
                    ..Default::default()
                },
                api_public_uri: format!("{base}/storage"),
                ..Default::default()
            },
            management: management_server::AppConfig {
                data_dir: root.join("management").to_string_lossy().into_owned(),
                server: ServerConfig {
                    prefix: Some("/management".into()),
                    ..Default::default()
                },
                api_public_uri: format!("{base}/management"),
                ..Default::default()
            },
        })
    }
}

impl LocalTlsConfig for DesktopConfig {
    fn tls_directory(&self) -> PathBuf {
        self.endpoint_data_dir.with_extension("tls")
    }

    fn tls_address(&self) -> SocketAddr {
        self.listen_addr
    }
}
