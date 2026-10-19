use std::{fs, io, path::Path, time::Duration};

use axum::http::HeaderValue;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;
#[cfg(unix)]
use unified_server::{BootstrapConfig, BootstrapPurpose, LocalTls, SetupHost};

#[cfg(unix)]
pub(crate) fn prepare(root: &Path) -> io::Result<SetupHost> {
    match fs::symlink_metadata(root) {
        Ok(_) => SetupHost::open(BootstrapConfig::open(root)?),
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
    pub(crate) async fn start(root: &Path) -> io::Result<Self> {
        let setup = prepare(root)?;
        Self::start_with(
            setup,
            |tls| crate::localhost_trust::install_ca_to_user_trust_store(&tls.ca_path()),
            LocalTls::verification_client,
        )
        .await
    }

    #[cfg(not(unix))]
    pub(crate) async fn start(_: &Path) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the shared native host requires Unix protected-state support; remote HTTPS APIs remain independent",
        ))
    }

    #[cfg(unix)]
    async fn start_with(
        setup: SetupHost,
        install: impl FnOnce(LocalTls) -> io::Result<()> + Send + 'static,
        client: impl FnOnce() -> io::Result<reqwest::Client>,
    ) -> io::Result<Self> {
        let tls = setup.tls().clone();
        tokio::task::spawn_blocking(move || install(tls))
            .await
            .map_err(io::Error::other)??;
        let client = client()?;
        let address = setup.local_addr();
        let listener = TcpListener::bind(address).await?;
        let tls = setup.tls().clone();
        let cancellation = CancellationToken::new();
        let worker = setup.into_worker(cancellation.clone());
        let router = worker.router_for_origin(native_origin())?;
        let shutdown = cancellation.clone();
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
            client,
            cancellation,
            task: Some(task),
        };
        let verification = host.verify().await;
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

    pub(crate) async fn status(&self) -> io::Result<serde_json::Value> {
        self.client
            .get(format!("{}/idp/setup/status", self.base_url))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json()
            .await
            .map_err(io::Error::other)
    }

    async fn verify(&self) -> io::Result<()> {
        for _ in 0..40 {
            if let Ok(response) = self
                .client
                .get(format!("{}/idp/setup/status", self.base_url))
                .timeout(Duration::from_secs(2))
                .send()
                .await
                && matches!(response.status().as_u16(), 200 | 503)
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err(io::Error::other(
            "native HTTPS host failed OS-root TLS verification",
        ))
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
