use std::{
    io,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use api::LocalTls;

use crate::{
    AppConfig,
    local_provisioning::{absolute, require_absent},
};

#[derive(Clone)]
pub struct BootstrapSettings {
    pub listen_addr: SocketAddr,
    pub endpoint_data_dir: std::path::PathBuf,
    pub idp: AppConfig,
    pub device: DeviceBootstrapSettings,
    pub storage: StorageBootstrapSettings,
}

#[derive(Clone)]
pub struct DeviceBootstrapSettings {
    pub data_dir: String,
    pub api_public_uri: String,
    pub storage_audience: String,
}

#[derive(Clone)]
pub struct StorageBootstrapSettings {
    pub data_dir: String,
    pub api_public_uri: String,
}

impl BootstrapSettings {
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

impl api::LocalTlsConfig for BootstrapSettings {
    fn tls_directory(&self) -> PathBuf {
        self.endpoint_data_dir.with_extension("tls")
    }
    fn tls_address(&self) -> SocketAddr {
        self.listen_addr
    }
}

fn roots(config: &BootstrapSettings) -> io::Result<[PathBuf; 7]> {
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

pub fn require_host_separation(config: &BootstrapSettings, host: &Path) -> io::Result<()> {
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
    config: &BootstrapSettings,
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
        crate::hosted_setup::setup_origin(&config.idp.ui_public_uri, false)?;
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

pub fn validate_config(config: &BootstrapSettings) -> io::Result<()> {
    if config.idp_audience().trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "IdP service audience must not be empty",
        ));
    }
    Ok(())
}
