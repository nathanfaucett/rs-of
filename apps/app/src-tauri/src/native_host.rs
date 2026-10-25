use std::{fs, io, path::Path, time::Duration};

use axum::http::HeaderValue;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;
#[cfg(unix)]
use unified_server::{BootstrapConfig, BootstrapPurpose, LocalTls, SetupHost};

#[cfg(unix)]
pub(crate) fn prepare(root: &Path) -> io::Result<SetupHost> {
    match fs::symlink_metadata(root) {
        Ok(_) => (|| SetupHost::open(BootstrapConfig::open(root)?))().map_err(|error| {
            if error.kind() == io::ErrorKind::PermissionDenied {
                error
            } else {
                io::Error::new(io::ErrorKind::InvalidData, error)
            }
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => SetupHost::create(
            BootstrapConfig::create(root, BootstrapPurpose::AwaitingChoice, None)?,
        ),
        Err(error) => Err(error),
    }
}

pub(crate) struct NativeHost {
    base_url: String,
    client: reqwest::Client,
    cancellation: CancellationToken,
    task: Option<JoinHandle<io::Result<()>>>,
}

impl NativeHost {
    #[cfg(unix)]
    pub(crate) async fn start(
        root: &Path,
        consent: bool,
        cancellation: CancellationToken,
    ) -> io::Result<Self> {
        let root = root.to_owned();
        let setup = tokio::task::spawn_blocking(move || prepare(&root))
            .await
            .map_err(io::Error::other)??;
        let stop = cancellation.clone();
        Self::start_cancellable(
            setup,
            move |tls| {
                if !consent {
                    return Err(consent_required());
                }
                crate::localhost_trust::install_ca_to_user_trust_store(&tls.ca_path(), &stop)
                    .map_err(|error| {
                        if error.kind() == io::ErrorKind::Interrupted {
                            error
                        } else {
                            io::Error::new(io::ErrorKind::PermissionDenied, error)
                        }
                    })
            },
            LocalTls::verification_client,
            cancellation,
        )
        .await
    }

    #[cfg(not(unix))]
    pub(crate) async fn start(_: &Path, _: bool, _: CancellationToken) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the shared native host requires Unix protected-state support; remote HTTPS APIs remain independent",
        ))
    }

    #[cfg(all(unix, test))]
    async fn start_with(
        setup: SetupHost,
        install: impl FnOnce(LocalTls) -> io::Result<()> + Send + 'static,
        client: impl FnMut() -> io::Result<reqwest::Client>,
    ) -> io::Result<Self> {
        Self::start_cancellable(setup, install, client, CancellationToken::new()).await
    }

    #[cfg(unix)]
    async fn start_cancellable(
        setup: SetupHost,
        install: impl FnOnce(LocalTls) -> io::Result<()> + Send + 'static,
        mut client: impl FnMut() -> io::Result<reqwest::Client>,
        cancellation: CancellationToken,
    ) -> io::Result<Self> {
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let initial_client = client()?;
        let address = setup.local_addr();
        let listener = TcpListener::bind(address).await?;
        let tls = setup.tls().clone();
        let worker = setup.into_worker(cancellation.clone());
        let router = worker.router_for_origin(native_origin())?;
        let shutdown = cancellation.clone();
        let install_tls = tls.clone();
        let task = tokio::spawn(async move {
            let _cancel_on_drop = shutdown.clone().drop_guard();
            let owner = tokio::spawn(worker.run());
            let result = axum::serve(tls.listener(listener), router)
                .with_graceful_shutdown(shutdown.clone().cancelled_owned())
                .await;
            shutdown.cancel();
            let cleanup = owner.await.map_err(io::Error::other)?;
            result.and(cleanup)
        });
        let mut host = Self {
            base_url: LocalTls::api_base(address)?,
            client: initial_client,
            cancellation,
            task: Some(task),
        };
        let verification = async {
            let verification = tokio::select! {
                biased;
                _ = host.cancellation.cancelled() => return Err(cancelled()),
                result = host.verify() => result,
            };
            match verification {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    if host.cancellation.is_cancelled() { return Err(cancelled()); }
                    tokio::task::spawn_blocking(move || install(install_tls))
                        .await.map_err(io::Error::other)??;
                    if host.cancellation.is_cancelled() { return Err(cancelled()); }
                    host.client = client()?;
                    tokio::select! {
                        biased;
                        _ = host.cancellation.cancelled() => Err(cancelled()),
                        result = host.verify() => result.map_err(|error| io::Error::other(format!("HTTPS verification after trust installation failed: {error}"))),
                    }
                }
                Err(error) => Err(error),
            }
        }.await;
        if let Err(error) = verification {
            if let Err(cleanup) = host.close().await {
                log::error!("native host cleanup after TLS verification failure: {cleanup}");
            }
            return Err(error);
        }
        Ok(host)
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) async fn status(&self) -> io::Result<of_idp_client::models::CreateResult> {
        let configuration = self.configuration();
        match of_idp_client::apis::setup_api::setup_status(&configuration).await {
            Ok(status) => Ok(status),
            Err(of_idp_client::apis::Error::ResponseError(response))
                if response.status.as_u16() == 503 =>
            {
                serde_json::from_str(&response.content).map_err(io::Error::other)
            }
            Err(error) => Err(io::Error::other(error)),
        }
    }

    pub(crate) async fn observation(&self, data: &Path) -> crate::onboarding::HostObservation {
        use crate::onboarding::{Failure, HostObservation};
        let mut observation = HostObservation {
            base: Some(self.base_url.clone()),
            ..HostObservation::default()
        };
        match self.status().await {
            Ok(status) => {
                if status.ready {
                    match crate::onboarding::login_client(&status).and_then(|client| {
                        crate::setup_completion::read(data, "device-setup-ready", client)
                    }) {
                        Ok(done) => observation.device_setup_required = !done,
                        Err(_) => observation.failure = Some(Failure::Repair),
                    }
                }
                observation.status = Some(status);
            }
            Err(_) => observation.failure = Some(Failure::Retry),
        }
        if self.cancellation.is_cancelled() {
            return HostObservation {
                failure: Some(Failure::Retry),
                ..HostObservation::default()
            };
        }
        observation
    }

    fn configuration(&self) -> of_idp_client::apis::configuration::Configuration {
        of_idp_client::apis::configuration::Configuration {
            base_path: format!("{}/idp", self.base_url),
            client: self.client.clone(),
            ..Default::default()
        }
    }

    async fn verify(&self) -> io::Result<()> {
        let response = self
            .client
            .request(
                reqwest::Method::OPTIONS,
                format!("{}/idp/setup/status", self.base_url),
            )
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map_err(|error| {
                let mut source: &(dyn std::error::Error + 'static) = &error;
                loop {
                    if matches!(
                        source.downcast_ref::<rustls::Error>(),
                        Some(rustls::Error::InvalidCertificate(
                            rustls::CertificateError::UnknownIssuer
                        ))
                    ) {
                        return io::Error::new(io::ErrorKind::PermissionDenied, error);
                    }
                    let nested = source
                        .downcast_ref::<io::Error>()
                        .and_then(io::Error::get_ref)
                        .map(|inner| inner as &(dyn std::error::Error + 'static));
                    match nested.or_else(|| source.source()) {
                        Some(next) => source = next,
                        None => return io::Error::other(error),
                    }
                }
            })?;
        if response.status() == reqwest::StatusCode::OK
            || response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED
        {
            Ok(())
        } else {
            Err(io::Error::other("native HTTPS status verification failed"))
        }
    }

    pub(crate) async fn close(&mut self) -> io::Result<()> {
        self.cancellation.cancel();
        if let Some(task) = self.task.take() {
            task.await.map_err(io::Error::other)??;
        }
        Ok(())
    }
}

impl Drop for NativeHost {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

fn native_origin() -> HeaderValue {
    #[cfg(dev)]
    {
        HeaderValue::from_static("https://idp-desktop.localhost:1355")
    }
    #[cfg(all(not(dev), target_os = "windows"))]
    {
        HeaderValue::from_static("http://tauri.localhost")
    }
    #[cfg(all(not(dev), not(target_os = "windows")))]
    {
        HeaderValue::from_static("tauri://localhost")
    }
}

#[cfg(all(test, unix))]
#[path = "desktop_lifecycle_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "native_provider_lifecycle_tests.rs"]
mod provider_lifecycle_tests;

#[derive(Debug)]
struct TrustConsentRequired;

impl core::fmt::Display for TrustConsentRequired {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("explicit certificate trust consent required")
    }
}
impl std::error::Error for TrustConsentRequired {}

pub(crate) fn consent_required() -> io::Error {
    io::Error::other(TrustConsentRequired)
}
pub(crate) fn needs_consent(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|source| source.is::<TrustConsentRequired>())
}
fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "native startup cancelled")
}
