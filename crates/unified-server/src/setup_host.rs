use std::{
    io,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use api::SetupAdmission;
#[cfg(unix)]
use axum::http::HeaderValue;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use tokio_util::sync::CancellationToken;

#[cfg(unix)]
use crate::{BootstrapConfig, BootstrapPurpose, local_provisioning::require_retained_directory};
use crate::{
    LocalTls, UnifiedConfig,
    local_provisioning::{absolute, require_absent},
};

#[cfg(unix)]
pub(crate) fn allowed_origin(value: &str) -> io::Result<HeaderValue> {
    idp_server::hosted_setup::setup_origin(value, false)
}

fn roots(config: &UnifiedConfig) -> io::Result<[PathBuf; 7]> {
    Ok([
        absolute(Path::new(&config.idp.data_dir))?,
        absolute(Path::new(&config.management.data_dir))?,
        absolute(Path::new(&config.storage.data_dir))?,
        absolute(&config.endpoint_data_dir)?,
        absolute(&config.credential_bootstrap_directory())?,
        absolute(&config.local_provisioning_directory())?,
        absolute(&LocalTls::directory(config))?,
    ])
}

pub(crate) fn require_host_separation(config: &UnifiedConfig, host: &Path) -> io::Result<()> {
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

pub(crate) fn require_fresh_roots(
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
        allowed_origin(&config.idp.ui_public_uri)?;
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

/// Awaiting-choice host only: no domain owners, grants, enrollment or completion.
#[cfg(unix)]
pub struct SetupHost {
    host: BootstrapConfig,
    admission: Arc<SetupAdmission>,
    tls: Option<LocalTls>,
}

#[cfg(unix)]
impl SetupHost {
    pub fn create(host: BootstrapConfig) -> io::Result<Self> {
        Self::prepare(host, true, true)
    }

    pub fn open(host: BootstrapConfig) -> io::Result<Self> {
        Self::prepare(host, false, true)
    }

    pub fn create_http(host: BootstrapConfig) -> io::Result<Self> {
        Self::prepare(host, true, false)
    }

    pub fn open_http(host: BootstrapConfig) -> io::Result<Self> {
        Self::prepare(host, false, false)
    }

    fn prepare(mut host: BootstrapConfig, fresh: bool, https: bool) -> io::Result<Self> {
        if host.purpose() != BootstrapPurpose::AwaitingChoice
            && !(matches!(
                host.purpose(),
                BootstrapPurpose::Create | BootstrapPurpose::Join
            ) && host.setup_attempted()
                && !fresh)
        {
            return Err(io::Error::other(
                "retained Create/Join is not a fresh setup host",
            ));
        }
        if fresh == host.setup_attempted() {
            return Err(io::Error::other(
                "setup production history requires existing-only continuation or exact recovery",
            ));
        }
        let config = host.config();
        if host.purpose() == BootstrapPurpose::AwaitingChoice {
            require_fresh_roots(&config, fresh, https)?;
        }
        require_host_separation(&config, host.directory())?;
        if https {
            api::setup_origin(&config.idp.ui_public_uri)?
        } else {
            allowed_origin(&config.idp.ui_public_uri)?
        };
        let admission_root = host.directory().join("setup-admission");
        if fresh {
            require_absent(&admission_root)?;
        } else {
            require_retained_directory(&admission_root)?;
        }
        if fresh {
            host.begin_setup()?;
        }
        let admission = Arc::new(SetupAdmission::open(&admission_root).map_err(io::Error::other)?);
        let tls = if !https {
            None
        } else if fresh {
            Some(LocalTls::create(&config)?)
        } else {
            Some(LocalTls::open(&config)?)
        };
        Ok(Self {
            host,
            admission,
            tls,
        })
    }

    /// Path only; code delivery belongs exclusively to protected host management.
    pub fn admission_path(&self) -> PathBuf {
        self.host.directory().join("setup-admission/state")
    }

    /// Native HTTPS material. Panics when called on an HTTP setup host.
    pub fn tls(&self) -> &LocalTls {
        self.tls.as_ref().expect("native HTTPS setup host")
    }

    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.host.config().listen_addr
    }

    /// Transfer the lifecycle guard and owner worker to an external listener host.
    /// The caller must preserve the selected transport and cancel/join the worker on shutdown.
    pub fn into_worker(self, cancellation: CancellationToken) -> crate::ApiSetupHost {
        crate::setup_runtime::setup_worker(self.host, self.tls, self.admission, cancellation)
    }

    pub async fn serve(self, cancellation: CancellationToken) -> io::Result<()> {
        let public_idp = self.host.config().idp.api_public_uri;
        let tls = self.tls.clone();
        let address = self.local_addr();
        let path = self.admission_path();
        let _cancel_on_drop = cancellation.clone().drop_guard();
        let create = crate::setup_runtime::setup_worker(
            self.host,
            self.tls,
            self.admission,
            cancellation.clone(),
        );
        let router = create.router()?;
        let owner = tokio::spawn(create.run());
        let result = if let Some(tls) = tls {
            api::serve_setup(
                (),
                tls,
                address,
                router,
                "/idp/setup/status".into(),
                path,
                cancellation.clone(),
            )
            .await
        } else {
            async {
                let listener = tokio::net::TcpListener::bind(address).await?;
                println!("Local setup HTTP: http://{}", listener.local_addr()?);
                println!("Public IdP: {public_idp}");
                println!(
                    "Protected management setup-code retrieval path: {}",
                    path.display()
                );
                axum::serve(listener, router.into_make_service())
                    .with_graceful_shutdown(cancellation.clone().cancelled_owned())
                    .await
            }
            .await
        };
        cancellation.cancel();
        let cleanup = owner.await.map_err(io::Error::other)?;
        result.and(cleanup)
    }

    #[cfg(test)]
    pub(crate) async fn serve_verified(
        self,
        client: reqwest::Client,
        cancellation: CancellationToken,
        announce: bool,
    ) -> io::Result<()> {
        let tls = self.tls.clone();
        let address = self.local_addr();
        let path = self.admission_path();
        let _cancel_on_drop = cancellation.clone().drop_guard();
        let create = crate::setup_runtime::setup_worker(
            self.host,
            self.tls,
            self.admission,
            cancellation.clone(),
        );
        let router = create.router()?;
        let owner = tokio::spawn(create.run());
        let result = api::serve_setup_verified(
            (),
            tls.expect("verified native HTTPS setup host"),
            address,
            router,
            "/idp/setup/status".into(),
            path,
            cancellation.clone(),
            client,
            announce,
        )
        .await;
        cancellation.cancel();
        let cleanup = owner.await.map_err(io::Error::other)?;
        result.and(cleanup)
    }
}
