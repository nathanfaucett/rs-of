use std::{
    fs::{File, TryLockError},
    io::{self, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::Router;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::canonical_openapi::canonical_openapi_router;
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
pub struct HostSettings {
    version: u8,
    pub owner: String,
    pub domain: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    address: Option<SocketAddr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) proxy_origin: Option<String>,
    pub origin: String,
    pub prefix: String,
    #[serde(default)]
    pub shared_tls: bool,
}

impl HostSettings {
    pub(crate) const fn is_native(&self) -> bool {
        self.address.is_some() && self.proxy_origin.is_none()
    }
}

impl LocalTlsConfig for HostSettings {
    fn tls_directory(&self) -> PathBuf {
        self.domain.with_extension("setup-host").join("tls")
    }
    fn tls_address(&self) -> SocketAddr {
        self.address
            .expect("native TLS requires a listener address")
    }
}

enum Transport {
    Native(SocketAddr, Option<LocalTls>),
    Proxy(String, reqwest::Client),
}

enum Serving {
    Native(LocalTls),
    Proxy(reqwest::Client),
}

/// An owner-local awaiting-choice host, not a provisioned context or completed Node.
pub struct IndependentSetupHost {
    pub(crate) settings: HostSettings,
    pub(crate) admission: Arc<SetupAdmission>,
    serving: Serving,
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
        Self::prepare_inner(
            owner,
            domain,
            origin,
            prefix,
            configured_identity,
            Transport::Native(address, None),
        )
    }

    /// Compose an owner behind the caller's one retained local TLS authority.
    pub fn prepare_with_tls(
        owner: &str,
        domain: &Path,
        address: SocketAddr,
        origin: &str,
        prefix: &str,
        tls: LocalTls,
    ) -> io::Result<Self> {
        Self::prepare_inner(owner, domain, origin, prefix, false, Transport::Native(address, Some(tls)))?
            .ok_or_else(|| io::Error::other("retained domain has no owner-local setup evidence; preserve it and explicitly reset or recover"))
    }

    /// Compose an owner behind a verified HTTPS proxy. The caller must bind the
    /// upstream to loopback and verify the public route before serving setup.
    ///
    /// # Errors
    /// Returns an error for invalid origins, conflicting retained state, or unavailable admission.
    pub fn prepare_for_proxy(
        owner: &str,
        domain: &Path,
        origin: &str,
        public_origin: &str,
        client: reqwest::Client,
    ) -> io::Result<Self> {
        if !public_origin.starts_with("https://") {
            return Err(io::Error::other("setup proxy requires HTTPS"));
        }
        setup_origin(public_origin)?;
        Self::prepare_inner(
            owner,
            domain,
            origin,
            &format!("/{owner}"),
            false,
            Transport::Proxy(public_origin.into(), client),
        )?
        .ok_or_else(|| io::Error::other("retained domain has no owner-local setup evidence"))
    }

    fn prepare_inner(
        owner: &str,
        domain: &Path,
        origin: &str,
        prefix: &str,
        configured_identity: bool,
        transport: Transport,
    ) -> io::Result<Option<Self>> {
        SetupScope::from_owners(&[owner])?;
        let domain = absolute(domain)?;
        let directory = domain.with_extension("setup-host");
        if present(&domain.with_extension("setup-reset"))?
            || present(&domain.with_extension("setup-reset.json"))?
        {
            return Err(io::Error::other(
                "interrupted reset requires explicit exact reset continuation",
            ));
        }
        if directory == domain || directory.starts_with(&domain) || domain.starts_with(&directory) {
            return Err(io::Error::other(
                "owner setup and domain roots must be separate",
            ));
        }
        let retained = present(&directory)?;
        let domain_exists = present(&domain)?;
        if domain_exists && !retained {
            return Ok(None);
        }
        if configured_identity {
            return Err(io::Error::other(
                "configured owner identity with missing domain state requires exact recovery",
            ));
        }
        let (address, proxy_origin, shared_tls) = match &transport {
            Transport::Native(address, tls) => {
                LocalTls::api_base(*address)?;
                (Some(*address), None, tls.is_some())
            }
            Transport::Proxy(origin, _) => (None, Some(origin.clone()), false),
        };
        setup_origin(origin)?;
        let prefix = prefix.trim_end_matches('/');
        if !prefix.is_empty() && prefix != format!("/{owner}") {
            return Err(io::Error::other(
                "independent setup prefix must be empty or the owner composition prefix",
            ));
        }

        let settings = HostSettings {
            version: if proxy_origin.is_some() { 2 } else { 1 },
            owner: owner.into(),
            domain,
            address,
            proxy_origin,
            origin: origin.into(),
            prefix: prefix.into(),
            shared_tls,
        };
        let operation = retain_settings(&settings, &directory, retained)?;
        let admission = Arc::new(SetupAdmission::open(&directory.join("setup-admission"))?);
        if domain_exists && admission.host_intent().map_err(io::Error::other)?.is_none() {
            return Err(io::Error::other(
                "domain state has no admitted exact setup operation",
            ));
        }
        let serving = match transport {
            Transport::Proxy(_, client) => Serving::Proxy(client),
            Transport::Native(_, tls) => Serving::Native(match tls {
                Some(tls) => tls,
                None if retained => LocalTls::open(&settings)?,
                None => LocalTls::create(&settings)?,
            }),
        };
        Ok(Some(Self {
            settings,
            admission,
            serving,
            _operation: operation,
        }))
    }

    pub fn require_owner(&self, owner: &str, domain: &Path) -> io::Result<()> {
        if self.settings.owner != owner || self.settings.domain != absolute(domain)? {
            return Err(io::Error::other(
                "setup worker does not own the retained domain root",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn admission_path(&self) -> PathBuf {
        self.settings
            .domain
            .with_extension("setup-host")
            .join("setup-admission/state")
    }

    /// # Panics
    /// Panics for a host prepared for a proxy.
    #[must_use]
    pub const fn tls(&self) -> &LocalTls {
        match &self.serving {
            Serving::Native(tls) => tls,
            Serving::Proxy(_) => panic!("proxy setup has no native TLS listener"),
        }
    }

    /// # Panics
    /// Panics for a host prepared for a proxy.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.settings
            .address
            .expect("native setup has a listener address")
    }

    pub(crate) fn native_listener(&self) -> io::Result<(LocalTls, SocketAddr)> {
        match &self.serving {
            Serving::Native(tls) => Ok((tls.clone(), self.local_addr())),
            Serving::Proxy(_) => Err(io::Error::other(
                "proxy setup requires caller-owned serving",
            )),
        }
    }

    pub(crate) fn base(&self) -> io::Result<String> {
        let origin = match &self.settings.proxy_origin {
            Some(origin) => origin.clone(),
            None => LocalTls::api_base(self.local_addr())?,
        };
        Ok(format!("{origin}{}", self.settings.prefix))
    }

    pub(crate) fn http_client(&self) -> io::Result<reqwest::Client> {
        match &self.serving {
            Serving::Native(tls) => tls.client(),
            Serving::Proxy(client) => Ok(client.clone()),
        }
    }

    pub fn router(&self) -> Router {
        let router = setup_router(
            self.admission.clone(),
            setup_origin(&self.settings.origin).expect("validated retained setup origin"),
            vec![self.settings.domain.clone()],
            SetupScope::from_owners(&[self.settings.owner.as_str()])
                .expect("validated retained owner scope"),
        )
        .merge(canonical_openapi_router(
            &self.settings.owner,
            &self.base().expect("validated owner base"),
        ));
        if self.settings.prefix.is_empty() {
            router
        } else {
            Router::new().nest(&self.settings.prefix, router)
        }
    }

    pub async fn serve(self, cancellation: CancellationToken) -> io::Result<()> {
        let router = self.router();
        let (tls, address) = self.native_listener()?;
        let path = format!("{}/setup/owner/status", self.settings.prefix);
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

fn retain_settings(settings: &HostSettings, directory: &Path, retained: bool) -> io::Result<File> {
    if !retained {
        require_absent(directory)?;
        create_private_directory(directory, true)?;
    }
    require_private(directory, true)?;
    let operation_path = directory.join("operation.lock");
    if retained {
        require_private(&operation_path, false)?;
    }
    let operation = private_options(!retained).open(&operation_path)?;
    operation.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => io::Error::new(
            io::ErrorKind::WouldBlock,
            "owner setup is already running; stop it before retrying",
        ),
        TryLockError::Error(error) => error,
    })?;
    if retained {
        let retained: HostSettings =
            serde_json::from_slice(&read_private(&directory.join("host.json"))?)?;
        if &retained != settings {
            return Err(io::Error::other(
                "retained owner setup settings disagree; exact continuation required",
            ));
        }
        require_private(&directory.join("setup-admission"), true)?;
    } else {
        operation.sync_all()?;
        sync_ancestry(directory)?;
        let mut file = private_options(true).open(directory.join("host.json"))?;
        file.write_all(&serde_json::to_vec(settings)?)?;
        file.sync_all()?;
        sync_ancestry(directory)?;
    }
    Ok(operation)
}
