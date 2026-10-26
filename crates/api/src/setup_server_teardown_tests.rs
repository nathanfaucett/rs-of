use std::{
    future::{Future, poll_fn},
    io,
    pin::Pin,
    sync::{Arc, Condvar, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use axum::serve::Listener;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tokio_util::sync::CancellationToken;

use crate::IndependentSetupHost;

struct TeardownGate {
    entered: Mutex<Option<oneshot::Sender<()>>>,
    released: Mutex<bool>,
    wake: Condvar,
}

struct Release(Arc<TeardownGate>);

impl Drop for Release {
    fn drop(&mut self) {
        *self.0.released.lock().expect("teardown gate") = true;
        self.0.wake.notify_all();
    }
}

struct GatedIo {
    stream: TcpStream,
    gate: Arc<TeardownGate>,
}

impl Drop for GatedIo {
    fn drop(&mut self) {
        if let Some(entered) = self
            .gate
            .entered
            .lock()
            .expect("teardown notification")
            .take()
        {
            let _ = entered.send(());
        }
        let mut released = self.gate.released.lock().expect("teardown gate");
        while !*released {
            released = self.gate.wake.wait(released).expect("release teardown");
        }
    }
}

impl AsyncRead for GatedIo {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for GatedIo {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}

struct GatedListener {
    listener: TcpListener,
    gate: Arc<TeardownGate>,
}

impl Listener for GatedListener {
    type Io = GatedIo;
    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let (stream, address) = self
            .listener
            .accept()
            .await
            .expect("accept real connection");
        (
            GatedIo {
                stream,
                gate: self.gate.clone(),
            },
            address,
        )
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener.local_addr()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_shutdown_joins_actual_connection_teardown_before_reopen() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let root = tempfile::tempdir().expect("isolated independent owner");
        let domain = root.path().join("device");
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let prepare = || IndependentSetupHost::prepare(
            "device", &domain, address, "https://localhost:5173", "", false,
        );
        let setup = prepare().expect("prepare").expect("independent host");
        let admission_path = setup.admission_path();
        let admission = std::fs::read(&admission_path).expect("retained admission");
        let shutdown = CancellationToken::new();
        let (host, _commands) = setup.into_create(shutdown.clone()).expect("Create host");
        let router = host.router();
        let (entered, mut teardown) = oneshot::channel();
        let gate = Arc::new(TeardownGate {
            entered: Mutex::new(Some(entered)),
            released: Mutex::new(false),
            wake: Condvar::new(),
        });
        let release = Release(gate.clone());
        let serving = super::serve_listener(host, GatedListener { listener, gate }, router, shutdown.clone());
        tokio::pin!(serving);
        let client = reqwest::Client::builder().http1_only().build().expect("HTTP client");
        let response = tokio::select! {
            result = &mut serving => panic!("listener ended before request: {result:?}"),
            response = client.get(format!("http://{address}/independent/setup/status")).send() => response.expect("real owner route"),
        };
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.bytes().await.expect("consume response, retaining keep-alive connection");
        shutdown.cancel();
        tokio::select! {
            result = &mut serving => panic!("shutdown escaped actual connection teardown: {result:?}"),
            result = &mut teardown => result.expect("connection reached destruction after Axum notification"),
        }
        poll_fn(|cx| {
            assert!(serving.as_mut().poll(cx).is_pending(), "shutdown must join actual connection destruction");
            Poll::Ready(())
        }).await;
        assert!(prepare().is_err(), "owner exclusion remains held during connection teardown");
        drop(release);
        serving.await.expect("joined listener and connection destruction");
        let reopened = prepare().expect("single immediate owner reopen").expect("retained host");
        assert_eq!(std::fs::read(reopened.admission_path()).expect("reopened admission"), admission);
    }).await.expect("bounded connection teardown regression");
}
