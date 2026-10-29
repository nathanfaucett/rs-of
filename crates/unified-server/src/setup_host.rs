#[cfg(unix)]
use std::sync::Arc;
use std::{io, path::PathBuf};

#[cfg(unix)]
use api::SetupAdmission;
#[cfg(unix)]
use axum::http::HeaderValue;
use idp_server::local_provisioning::{require_absent, require_retained_directory};
#[cfg(unix)]
use tokio_util::sync::CancellationToken;

use crate::LocalTls;
#[cfg(unix)]
use crate::{BootstrapConfig, BootstrapPurpose};

#[cfg(unix)]
pub(crate) fn allowed_origin(value: &str) -> io::Result<HeaderValue> {
    idp_server::hosted_setup::setup_origin(value, false)
}

pub(crate) use crate::config::{require_fresh_roots, require_host_separation};

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
        let store = host.store();
        if fresh {
            store.put("admission", &serde_json::json!({"retired": false}))?;
        }
        let admission = Arc::new(SetupAdmission::open_with_history(
            &admission_root,
            Arc::new(store),
        )?);
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

    /// Existing-only protected retrieval; SQL retirement overrides leftover code material.
    pub fn protected_code(
        root: &std::path::Path,
    ) -> Result<Option<String>, api::SetupAdmissionError> {
        let parent = root.parent().ok_or(api::SetupAdmissionError::Unavailable)?;
        let store = crate::host_store::HostStore::open(parent)
            .map_err(|_| api::SetupAdmissionError::Unavailable)?;
        SetupAdmission::protected_code_with_history(root, Arc::new(store))
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
        let public_idp = self.host.config().idp.api_public_uri.clone();
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
