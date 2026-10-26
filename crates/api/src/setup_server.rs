use std::{
    convert::Infallible,
    future::ready,
    io,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use axum::{Router, extract::Request, http::StatusCode};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::{
    LocalTls,
    setup_install::{CancelInstallation, Installation},
};

/// Owns the caller's lifecycle guard until the listener and its connection services are destroyed.
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
    server.spawn(async move { serve_listener(guard, listener, router, shutdown).await });
    let verified = async {
        // Verify native TLS and HTTP serving without running current remote readiness.
        let response = client
            .request(reqwest::Method::OPTIONS, format!("{base}{status_path}"))
            .send()
            .await
            .map_err(|_| io::Error::other("setup native HTTPS verification failed"))?;
        if !matches!(
            response.status(),
            StatusCode::OK | StatusCode::METHOD_NOT_ALLOWED
        ) {
            return Err(io::Error::other("setup HTTPS status verification failed"));
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

#[derive(Clone)]
struct SetupConnection {
    router: Router,
    // Never sent; last field so closure follows destruction of the real router.
    _teardown: watch::Sender<()>,
}

impl tower::Service<Request> for SetupConnection {
    type Response = axum::response::Response;
    type Error = Infallible;
    type Future = <Router as tower::Service<Request>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Router as tower::Service<Request>>::poll_ready(&mut self.router, cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        self.router.call(request)
    }
}

async fn serve_listener<G, L>(
    guard: G,
    listener: L,
    router: Router,
    shutdown: CancellationToken,
) -> io::Result<()>
where
    G: Send + 'static,
    L: axum::serve::Listener,
    L::Addr: std::fmt::Debug,
{
    let _guard = guard;
    let (teardown, mut destroyed) = watch::channel(());
    let connection = SetupConnection {
        router,
        _teardown: teardown,
    };
    let make_service = tower::service_fn(move |_: axum::serve::IncomingStream<'_, L>| {
        ready(Ok::<_, Infallible>(connection.clone()))
    });
    let result = axum::serve(listener, make_service)
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await;
    let _ = destroyed.changed().await;
    result
}

#[cfg(test)]
#[path = "setup_server_teardown_tests.rs"]
mod teardown_tests;

#[cfg(test)]
mod tests {

    #[test]
    fn listener_survives_readiness_slower_than_native_verification_deadline() {
        use std::{
            fs,
            process::Command,
            time::{Duration, Instant},
        };
        let root = tempfile::tempdir().expect("isolated root");
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve");
        let address = reserved.local_addr().expect("address");
        let host = crate::IndependentSetupHost::prepare(
            "storage",
            &root.path().join("storage"),
            address,
            "https://localhost:5173",
            "/storage",
            false,
        )
        .expect("prepare")
        .expect("host");
        fs::create_dir(root.path().join("empty-roots")).expect("isolated roots");
        let ca = host.tls().ca_path();
        drop(host);
        drop(reserved);
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "setup_server::tests::delayed_readiness_child",
                "--ignored",
                "--nocapture",
            ])
            .env("OF_LISTENER_TEST_ROOT", root.path())
            .env("OF_LISTENER_TEST_ADDRESS", address.to_string())
            .env("SSL_CERT_FILE", ca)
            .env("SSL_CERT_DIR", root.path().join("empty-roots"))
            .spawn()
            .expect("isolated native-root child");
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = child.try_wait().expect("child status") {
                assert!(status.success());
                break;
            }
            if Instant::now() > deadline {
                child.kill().expect("bound child");
                child.wait().expect("reap child");
                panic!("listener child exceeded bound");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[tokio::test]
    #[ignore = "parent supplies isolated native roots"]
    async fn delayed_readiness_child() {
        use axum::{Router, http::StatusCode, routing::get};
        use std::{
            path::PathBuf,
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
            time::Duration,
        };
        use tokio_util::sync::CancellationToken;
        let root = PathBuf::from(std::env::var_os("OF_LISTENER_TEST_ROOT").expect("root"));
        let address = std::env::var("OF_LISTENER_TEST_ADDRESS")
            .expect("address")
            .parse()
            .expect("address");
        let host = crate::IndependentSetupHost::prepare(
            "storage",
            &root.join("storage"),
            address,
            "https://localhost:5173",
            "/storage",
            false,
        )
        .expect("retained prepare")
        .expect("host");
        let tls = host.tls().clone();
        let unavailable = Arc::new(AtomicBool::new(true));
        let router = Router::new().route("/storage/setup/status", get({
            let unavailable = unavailable.clone();
            move || { let unavailable = unavailable.clone(); async move {
                if unavailable.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_secs(6)).await;
                    (StatusCode::SERVICE_UNAVAILABLE, axum::Json(serde_json::json!({"stage":"installation", "state":"unavailable", "ready":false, "owners":["storage"]})))
                } else {
                    (StatusCode::OK, axum::Json(serde_json::json!({"stage":"installation", "state":"ready", "ready":true, "owners":["storage"]})))
                }
            }}
        })).route("/storage/databases", get(|| async { StatusCode::UNAUTHORIZED }));
        let cancellation = CancellationToken::new();
        let client = crate::LocalTls::verification_client().expect("native roots");
        let server = tokio::spawn(super::serve_setup_inner(
            host,
            tls,
            address,
            router,
            "/storage/setup/status".into(),
            root.join("admission"),
            cancellation.clone(),
            client.clone(),
            false,
        ));
        tokio::time::sleep(Duration::from_secs(7)).await;
        assert!(
            !server.is_finished(),
            "remote readiness must not cancel a TLS-verified listener"
        );
        unavailable.store(false, Ordering::SeqCst);
        let base = format!("https://{address}/storage");
        assert_eq!(
            client
                .get(format!("{base}/setup/status"))
                .send()
                .await
                .expect("recovery without reseed")
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            client
                .get(format!("{base}/databases"))
                .send()
                .await
                .expect("normal auth")
                .status(),
            StatusCode::UNAUTHORIZED
        );
        cancellation.cancel();
        server
            .await
            .expect("listener join")
            .expect("clean shutdown");
    }
}
