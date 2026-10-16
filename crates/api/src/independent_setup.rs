use std::{
    fs::File,
    io::{self, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::Router;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{
    LocalTls, LocalTlsConfig, SetupAdmission, SetupScope, serve_setup,
    setup_files::{
        absolute, create_private_directory, present, private_options, read_private, require_absent,
        require_private, sync_ancestry,
    },
    setup_origin, setup_router,
};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct HostSettings {
    version: u8,
    owner: String,
    domain: PathBuf,
    address: SocketAddr,
    origin: String,
    prefix: String,
}

impl LocalTlsConfig for HostSettings {
    fn tls_directory(&self) -> PathBuf {
        self.domain.with_extension("setup-host").join("tls")
    }
    fn tls_address(&self) -> SocketAddr {
        self.address
    }
}

/// An owner-local awaiting-choice host, not a provisioned context or completed Node.
pub struct IndependentSetupHost {
    settings: HostSettings,
    admission: Arc<SetupAdmission>,
    tls: LocalTls,
    _operation: File,
}

impl IndependentSetupHost {
    /// Existing domain state keeps its ordinary authenticated startup. A retained
    /// setup root must reopen exactly; absence of its admission is never fresh permission.
    pub fn prepare(
        owner: &str,
        domain: &Path,
        address: SocketAddr,
        origin: &str,
        prefix: &str,
        configured_identity: bool,
    ) -> io::Result<Option<Self>> {
        SetupScope::from_owners(&[owner])?;
        let domain = absolute(domain)?;
        let directory = domain.with_extension("setup-host");
        if directory == domain || directory.starts_with(&domain) || domain.starts_with(&directory) {
            return Err(io::Error::other(
                "owner setup and domain roots must be separate",
            ));
        }
        let retained = present(&directory)?;
        let domain_exists = present(&domain)?;
        if domain_exists {
            if retained {
                return Err(io::Error::other(
                    "retained setup with domain state requires explicit provisioning continuation",
                ));
            }
            return Ok(None);
        }
        if configured_identity {
            return Err(io::Error::other(
                "configured owner identity with missing domain state requires exact recovery",
            ));
        }
        LocalTls::api_base(address)?;
        setup_origin(origin)?;
        let prefix = prefix.trim_end_matches('/');
        if !prefix.is_empty() && prefix != format!("/{owner}") {
            return Err(io::Error::other(
                "independent setup prefix must be empty or the owner composition prefix",
            ));
        }

        let settings = HostSettings {
            version: 1,
            owner: owner.into(),
            domain,
            address,
            origin: origin.into(),
            prefix: prefix.into(),
        };
        if !retained {
            require_absent(&directory)?;
            create_private_directory(&directory, true)?;
        }
        require_private(&directory, true)?;
        let operation_path = directory.join("operation.lock");
        if retained {
            require_private(&operation_path, false)?;
        }
        let operation = private_options(!retained).open(&operation_path)?;
        operation.try_lock().map_err(io::Error::other)?;
        if !retained {
            operation.sync_all()?;
            sync_ancestry(&directory)?;
            let mut file = private_options(true).open(directory.join("host.json"))?;
            file.write_all(&serde_json::to_vec(&settings)?)?;
            file.sync_all()?;
            sync_ancestry(&directory)?;
        } else {
            let retained: HostSettings =
                serde_json::from_slice(&read_private(&directory.join("host.json"))?)?;
            if retained != settings {
                return Err(io::Error::other(
                    "retained owner setup settings disagree; exact continuation required",
                ));
            }
            require_private(&directory.join("setup-admission"), true)?;
        }
        let admission = Arc::new(
            SetupAdmission::open(&directory.join("setup-admission")).map_err(io::Error::other)?,
        );
        let tls = if retained {
            LocalTls::open(&settings)?
        } else {
            LocalTls::create(&settings)?
        };
        Ok(Some(Self {
            settings,
            admission,
            tls,
            _operation: operation,
        }))
    }

    pub fn admission_path(&self) -> PathBuf {
        self.settings
            .domain
            .with_extension("setup-host")
            .join("setup-admission/state")
    }

    pub fn tls(&self) -> &LocalTls {
        &self.tls
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.settings.address
    }

    pub fn router(&self) -> Router {
        let router = setup_router(
            self.admission.clone(),
            setup_origin(&self.settings.origin).expect("validated retained setup origin"),
            vec![self.settings.domain.clone()],
            SetupScope::from_owners(&[self.settings.owner.as_str()])
                .expect("validated retained owner scope"),
        );
        if self.settings.prefix.is_empty() {
            router
        } else {
            Router::new().nest(&self.settings.prefix, router)
        }
    }

    pub async fn serve(self, cancellation: CancellationToken) -> io::Result<()> {
        let router = self.router();
        let tls = self.tls.clone();
        let address = self.local_addr();
        let path = format!("{}/setup/status", self.settings.prefix);
        let admission_path = self.admission_path();
        serve_setup(
            self,
            tls,
            address,
            router,
            path,
            admission_path,
            cancellation,
        )
        .await
    }
}
