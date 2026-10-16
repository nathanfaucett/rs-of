use std::{
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
};

use idp_model::model::Id;
use serde::{Deserialize, Serialize};

use crate::{
    UnifiedConfig,
    local_provisioning::{
        absolute, create_private_directory, lock, private_options, read_private, require_absent,
        sync_ancestry,
    },
};

#[derive(Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct References {
    version: u8,
    tls_attempted: bool,
    roots: [PathBuf; 4],
    issuer: String,
    key_namespace: String,
    applications: [String; 3],
    idp_audience: String,
    storage_audience: String,
}

/// Durable host references and consumed TLS production history; never contains raw secrets.
/// Hold this guard through setup or serving to exclude cooperating CLI operations.
pub struct CredentialBootstrap {
    config: UnifiedConfig,
    directory: PathBuf,
    references: References,
    _operation: File,
}

impl CredentialBootstrap {
    /// Explicit fresh authorization. Context, endpoint, provisioning and credential roots must be absent.
    pub fn create(config: UnifiedConfig) -> io::Result<Self> {
        Self::create_inner(config, false)
    }

    pub(crate) fn adopt_host_tls(config: UnifiedConfig, tls: &crate::LocalTls) -> io::Result<Self> {
        let expected = absolute(&crate::LocalTls::directory(&config))?.join("ca.pem");
        if tls.ca_path() != expected {
            return Err(invalid("neutral TLS does not belong to the selected host"));
        }
        crate::LocalTls::open(&config)?;
        Self::create_inner(config, true)
    }

    fn create_inner(config: UnifiedConfig, retained_tls: bool) -> io::Result<Self> {
        crate::local_provisioning::require_unix_bootstrap()?;
        crate::LocalTls::validate_ip(config.listen_addr.ip())?;
        let mut references = references(&config)?;
        references.tls_attempted = retained_tls;
        let directory = absolute(&config.credential_bootstrap_directory())?;
        let provisioning = absolute(&config.local_provisioning_directory())?;
        let tls = absolute(&crate::LocalTls::directory(&config))?;
        let roots = references
            .roots
            .iter()
            .chain([&directory, &provisioning, &tls])
            .collect::<Vec<_>>();
        for (index, root) in roots.iter().enumerate() {
            if !retained_tls || *root != &tls {
                require_absent(root)?;
            }
            if roots[..index]
                .iter()
                .any(|other| root.starts_with(other) || other.starts_with(root))
            {
                return Err(invalid("bootstrap and context roots must not overlap"));
            }
        }
        if config.idp.oauth2.local_signer.is_some() || !config.listen_addr.ip().is_loopback() {
            return Err(invalid(
                "fresh bootstrap requires loopback and no configured Node signer",
            ));
        }
        create_private_directory(
            directory.parent().expect("absolute credential parent"),
            true,
        )?;
        create_private_directory(&directory, false)?;
        let operation = lock(&directory, true)?;
        let bootstrap = Self {
            config,
            directory,
            references,
            _operation: operation,
        };
        bootstrap.write()?;
        Ok(bootstrap)
    }

    /// Opens exact public references only; missing or malformed state is never fresh authorization.
    pub fn open(config: UnifiedConfig) -> io::Result<Self> {
        let directory = absolute(&config.credential_bootstrap_directory())?;
        let operation = lock(&directory, false)?;
        let bytes = read_private(&directory.join("credentials.json"))?;
        let retained: References = serde_json::from_slice(&bytes)?;
        let mut expected = references(&config)?;
        expected.tls_attempted = retained.tls_attempted;
        if retained != expected
            || serde_json::to_value(&retained)?
                != serde_json::from_slice::<serde_json::Value>(&bytes)?
        {
            return Err(invalid(
                "invalid or conflicting credential bootstrap intent; exact recovery required",
            ));
        }
        Ok(Self {
            config,
            directory,
            references: retained,
            _operation: operation,
        })
    }

    /// Mandatory OS trust installation before owner bootstrap; missing attempted TLS is not replaced.
    pub fn prepare_tls(&mut self) -> io::Result<crate::LocalTls> {
        self.check_retained()?;
        crate::local_provisioning::require_unix_bootstrap()?;
        crate::LocalTls::api_base(self.config.listen_addr)?;
        let tls = if self.references.tls_attempted {
            crate::LocalTls::open(&self.config)?
        } else {
            self.references.tls_attempted = true;
            self.write()?;
            crate::LocalTls::create(&self.config)?
        };
        tls.install_os_ca()?;
        Ok(tls)
    }

    pub fn load(&self) -> io::Result<UnifiedConfig> {
        self.check_retained()?;
        crate::local_provisioning::normalized_config(self.config.clone())
    }

    fn check_retained(&self) -> io::Result<()> {
        if read_private(&self.directory.join("credentials.json"))?
            != serde_json::to_vec(&self.references)?
        {
            return Err(invalid("retained credential intent changed while guarded"));
        }
        Ok(())
    }

    fn write(&self) -> io::Result<()> {
        let temporary = self.directory.join(format!(".{}.tmp", Id::now_v7()));
        let bytes = serde_json::to_vec(&self.references)?;
        let mut file = private_options(true).open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(temporary, self.directory.join("credentials.json"))?;
        sync_ancestry(&self.directory)?;
        if read_private(&self.directory.join("credentials.json"))? != bytes {
            return Err(invalid("credential intent durable readback did not match"));
        }
        Ok(())
    }
}

fn references(config: &UnifiedConfig) -> io::Result<References> {
    Ok(References {
        version: 1,
        tls_attempted: false,
        roots: [
            absolute(std::path::Path::new(&config.idp.data_dir))?,
            absolute(std::path::Path::new(&config.management.data_dir))?,
            absolute(std::path::Path::new(&config.storage.data_dir))?,
            absolute(&config.endpoint_data_dir)?,
        ],
        issuer: config.idp.oauth2.issuer.clone(),
        key_namespace: config.idp.key_namespace.clone(),
        idp_audience: config.idp_audience().to_owned(),
        storage_audience: config.management.storage_audience.clone(),
        applications: [
            config.idp.api_public_uri.clone(),
            config.management.api_public_uri.clone(),
            config.storage.api_public_uri.clone(),
        ],
    })
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
