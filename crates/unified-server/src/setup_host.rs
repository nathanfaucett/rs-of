use std::{
    io,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use api::{SetupAdmission, SetupScope, setup_origin as allowed_origin, setup_router};
#[cfg(unix)]
use axum::{Router, http::HeaderValue};
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

pub(crate) fn require_fresh_roots(config: &UnifiedConfig, tls_absent: bool) -> io::Result<()> {
    if config.idp.oauth2.local_signer.is_some() {
        return Err(io::Error::other(
            "neutral startup requires no configured Node signer",
        ));
    }
    LocalTls::api_base(config.listen_addr)?;
    #[cfg(unix)]
    allowed_origin(&config.idp.ui_public_uri)?;
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
    tls: LocalTls,
    origin: HeaderValue,
}

#[cfg(unix)]
impl SetupHost {
    pub fn create(host: BootstrapConfig) -> io::Result<Self> {
        Self::prepare(host, true)
    }

    pub fn open(host: BootstrapConfig) -> io::Result<Self> {
        Self::prepare(host, false)
    }

    fn prepare(mut host: BootstrapConfig, fresh: bool) -> io::Result<Self> {
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
            require_fresh_roots(&config, fresh)?;
        }
        require_host_separation(&config, host.directory())?;
        let origin = allowed_origin(&config.idp.ui_public_uri)?;
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
        let tls = if fresh {
            LocalTls::create(&config)?
        } else {
            LocalTls::open(&config)?
        };
        Ok(Self {
            host,
            admission,
            tls,
            origin,
        })
    }

    /// Path only; code delivery belongs exclusively to protected host management.
    pub fn admission_path(&self) -> PathBuf {
        self.host.directory().join("setup-admission/state")
    }

    pub fn tls(&self) -> &LocalTls {
        &self.tls
    }

    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.host.config().listen_addr
    }

    /// Mount only on this host's TLS listener, never the ordinary resource router.
    pub fn router(&self) -> Router {
        let config = self.host.config();
        let absent_roots = vec![
            PathBuf::from(&config.idp.data_dir),
            PathBuf::from(&config.management.data_dir),
            PathBuf::from(&config.storage.data_dir),
            config.endpoint_data_dir.clone(),
            config.credential_bootstrap_directory(),
            config.local_provisioning_directory(),
        ];
        Router::new().nest(
            "/idp",
            setup_router(
                self.admission.clone(),
                self.origin.clone(),
                absent_roots,
                SetupScope::Unified,
            ),
        )
    }

    pub async fn serve(self, cancellation: CancellationToken) -> io::Result<()> {
        let tls = self.tls.clone();
        let address = self.local_addr();
        let path = self.admission_path();
        let _cancel_on_drop = cancellation.clone().drop_guard();
        let create = crate::api_create::CreateHost::new(
            self.host,
            self.tls,
            self.admission,
            cancellation.clone(),
        );
        let router = create.router()?;
        let owner = tokio::spawn(create.run());
        let result = api::serve_setup(
            (),
            tls,
            address,
            router,
            "/idp/setup/status".into(),
            path,
            cancellation.clone(),
        )
        .await;
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
        let create = crate::api_create::CreateHost::new(
            self.host,
            self.tls,
            self.admission,
            cancellation.clone(),
        );
        let router = create.router()?;
        let owner = tokio::spawn(create.run());
        let result = api::serve_setup_verified(
            (),
            tls,
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
