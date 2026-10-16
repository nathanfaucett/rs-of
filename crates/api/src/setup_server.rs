use std::{io, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use axum::{Router, http::StatusCode};
use tokio_util::sync::CancellationToken;

use crate::{
    LocalTls,
    setup_install::{CancelInstallation, Installation},
    setup_routes::SetupStatus,
};

/// Owns the caller's lifecycle guard until the listener has stopped.
#[cfg_attr(
    not(feature = "test-support"),
    doc = "```compile_fail\nuse api::serve_setup_verified;\n```"
)]
pub async fn serve_setup<G: Send + 'static>(
    guard: G,
    tls: LocalTls,
    address: SocketAddr,
    router: Router,
    status_path: String,
    admission_path: PathBuf,
    cancellation: CancellationToken,
) -> io::Result<()> {
    let installation = Arc::new(Installation::default());
    let _cancel_on_drop = CancelInstallation::new(installation.clone(), cancellation.clone());
    if cancellation.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "setup installation cancelled",
        ));
    }
    let install = tls.clone();
    let executor = installation.clone();
    let mut worker = tokio::task::spawn_blocking(move || {
        let result = install.install_os_ca_with(|command| executor.run(command));
        (guard, result)
    });
    let result = tokio::select! {
        biased;
        () = cancellation.cancelled() => {
            installation.cancel();
            worker.await
        }
        result = &mut worker => result,
    };
    let (guard, installed) =
        result.map_err(|_| io::Error::other("setup installation worker failed"))?;
    if cancellation.is_cancelled() {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "setup installation cancelled",
        ));
    }
    installed?;
    let client = LocalTls::verification_client()?;
    serve_setup_inner(
        guard,
        tls,
        address,
        router,
        status_path,
        admission_path,
        cancellation,
        client,
        true,
    )
    .await
}

#[cfg(feature = "test-support")]
#[allow(clippy::too_many_arguments)]
pub async fn serve_setup_verified<G: Send + 'static>(
    guard: G,
    tls: LocalTls,
    address: SocketAddr,
    router: Router,
    status_path: String,
    admission_path: PathBuf,
    cancellation: CancellationToken,
    client: reqwest::Client,
    announce: bool,
) -> io::Result<()> {
    serve_setup_inner(
        guard,
        tls,
        address,
        router,
        status_path,
        admission_path,
        cancellation,
        client,
        announce,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn serve_setup_inner<G: Send + 'static>(
    guard: G,
    tls: LocalTls,
    address: SocketAddr,
    router: Router,
    status_path: String,
    admission_path: PathBuf,
    cancellation: CancellationToken,
    client: reqwest::Client,
    announce: bool,
) -> io::Result<()> {
    let _cancel_on_drop = cancellation.clone().drop_guard();
    let listener = tokio::net::TcpListener::bind(address).await?;
    let base = LocalTls::api_base(listener.local_addr()?)?;
    let shutdown = cancellation.clone();
    let listener = tls.listener(listener);
    let mut server = tokio::task::JoinSet::new();
    server.spawn(async move {
        let _guard = guard;
        axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
    });
    let verified = async {
        let response = client
            .get(format!("{base}{status_path}"))
            .send()
            .await
            .map_err(|_| io::Error::other("setup native HTTPS verification failed"))?;
        if response.status() != StatusCode::OK {
            return Err(io::Error::other("setup HTTPS status verification failed"));
        }
        let status: SetupStatus = response
            .json()
            .await
            .map_err(|_| io::Error::other("invalid setup status"))?;
        if !valid_status(&status) {
            return Err(io::Error::other(
                "setup listener has inconsistent lifecycle status",
            ));
        }
        Ok(())
    }
    .await;
    if verified.is_ok() {
        if announce {
            println!("Local setup HTTPS: {base}");
            println!(
                "Protected management setup-code retrieval path: {}",
                admission_path.display()
            );
        }
        tokio::select! {
            result = server.join_next() => return result.expect("one setup listener task").map_err(io::Error::other)?,
            () = cancellation.cancelled() => (),
        }
    }
    cancellation.cancel();
    let stopped = match tokio::time::timeout(Duration::from_secs(10), server.join_next()).await {
        Ok(result) => result
            .expect("one setup listener task")
            .map_err(io::Error::other)
            .and_then(|result| result),
        Err(_) => {
            server.abort_all();
            let _ = server.join_next().await;
            Err(io::Error::other("setup HTTPS shutdown timed out"))
        }
    };
    verified.and(stopped)
}

fn valid_status(status: &SetupStatus) -> bool {
    status.stage == "installation"
        && matches!(
            status.state.as_str(),
            "awaiting_choice" | "unfinished" | "creating" | "joining" | "ready"
        )
        && status.ready == (status.state == "ready")
}

#[cfg(test)]
mod tests {
    use super::valid_status;
    use crate::setup_routes::SetupStatus;

    #[test]
    fn startup_verification_accepts_in_progress_join_but_not_false_readiness() {
        let mut status = SetupStatus {
            stage: "installation".into(),
            state: "joining".into(),
            ready: false,
            owners: vec!["idp".into(), "management".into(), "storage".into()],
        };
        assert!(valid_status(&status));
        status.ready = true;
        assert!(!valid_status(&status));
        status.state = "ready".into();
        assert!(valid_status(&status));
        status.state = "unknown".into();
        assert!(!valid_status(&status));
        status.state = "joining".into();
        status.ready = false;
        status.stage = "unknown".into();
        assert!(!valid_status(&status));
    }
}
