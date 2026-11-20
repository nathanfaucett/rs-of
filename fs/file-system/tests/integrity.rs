use std::{
    collections::VecDeque,
    env, fs,
    future::{pending, poll_fn, ready},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
};

use bytes::Bytes;
use file_system::{
    Error, FileFuture, FileHandle, FileSessionService, FileSystem, IncomingMessage, MemoryNetwork,
    MemoryTransport, OpenRequest, OperationControl, Residency, ScanPage, SessionRequest,
    SyncMessage, Transport,
};
use tokio::sync::{Notify, broadcast};
use uuid::Uuid;

#[derive(Clone)]
enum Chunk {
    Bytes(Bytes),
    Pending,
    Wait(Arc<Notify>),
    Error,
    End,
}

#[derive(Clone)]
struct Exchange {
    inner: MemoryTransport<u8>,
    chunks: Arc<Mutex<VecDeque<Chunk>>>,
    dropped: Arc<Mutex<usize>>,
}

struct Handle {
    exchange: Exchange,
}
impl Drop for Handle {
    fn drop(&mut self) {
        *self.exchange.dropped.lock().expect("handle counter") += 1;
    }
}
impl FileHandle<u8> for Handle {
    type Error = Error;
    fn read(&mut self, _: u64, _: u32) -> FileFuture<'_, Result<Bytes, Error>> {
        Box::pin(async move {
            let chunk = self
                .exchange
                .chunks
                .lock()
                .expect("chunks")
                .pop_front()
                .expect("script has next read");
            match chunk {
                Chunk::Bytes(bytes) => Ok(bytes),
                Chunk::Pending => pending().await,
                Chunk::Wait(notify) => {
                    notify.notified().await;
                    Ok(Bytes::new())
                }
                Chunk::Error => Err(Error::Offline),
                Chunk::End => Ok(Bytes::new()),
            }
        })
    }
    fn write(&mut self, _: u64, _: Bytes) -> FileFuture<'_, Result<u32, Error>> {
        Box::pin(async { Err(Error::Offline) })
    }
    fn scan(&mut self, _: Option<String>, _: u32) -> FileFuture<'_, Result<ScanPage<u8>, Error>> {
        Box::pin(async { Err(Error::Offline) })
    }
    fn close(self: Box<Self>) -> FileFuture<'static, Result<(), Error>> {
        Box::pin(async { Ok(()) })
    }
}
impl Transport<u8> for Exchange {
    type Error = Error;
    fn peers(&self) -> Vec<u8> {
        self.inner.peers()
    }
    async fn send(&self, peer: u8, message: SyncMessage<u8>) -> Result<(), Error> {
        self.inner
            .send(peer, message)
            .await
            .map_err(|_error| Error::Offline)
    }
    async fn broadcast(&self, message: SyncMessage<u8>) -> Result<(), Error> {
        self.inner
            .broadcast(message)
            .await
            .map_err(|_error| Error::Offline)
    }
    fn subscribe(&self) -> broadcast::Receiver<IncomingMessage<u8>> {
        self.inner.subscribe()
    }
}
impl FileSessionService<u8> for Exchange {
    type Handle = Handle;
    fn open(&self, _: u8, _: OpenRequest) -> impl Future<Output = Result<Handle, Error>> + Send {
        ready(Ok(Handle {
            exchange: self.clone(),
        }))
    }
    fn subscribe_file_sessions(&self) -> broadcast::Receiver<SessionRequest<u8>> {
        self.inner.subscribe_file_sessions()
    }
}

struct Fixture {
    source_root: PathBuf,
    target_root: PathBuf,
    source: FileSystem<u8>,
    target: FileSystem<u8>,
    source_transport: MemoryTransport<u8>,
    exchange: Exchange,
}
impl Fixture {
    async fn new(chunks: Vec<Chunk>) -> Self {
        let source_root = env::temp_dir().join(format!("offs-integrity-{}", Uuid::now_v7()));
        let target_root = env::temp_dir().join(format!("offs-integrity-{}", Uuid::now_v7()));
        let source = FileSystem::open(&source_root, 1)
            .await
            .expect("source opens");
        let target = FileSystem::open(&target_root, 2)
            .await
            .expect("target opens");
        source
            .set_residency("", Residency::Full)
            .await
            .expect("source full");
        target
            .set_residency("", Residency::Full)
            .await
            .expect("target full");
        source
            .write("file", b"complete")
            .await
            .expect("source write");
        let network = MemoryNetwork::new(32);
        let source_transport = network.transport(1);
        let exchange = Exchange {
            inner: network.transport(2),
            chunks: Arc::new(Mutex::new(chunks.into())),
            dropped: Arc::new(Mutex::new(0)),
        };
        Self {
            source_root,
            target_root,
            source,
            target,
            source_transport,
            exchange,
        }
    }
    async fn metadata(&self, sync: &mut file_system::MetadataSync<'_, u8, Exchange>) {
        self.source
            .metadata_sync(self.source_transport.clone())
            .announce()
            .await
            .expect("metadata queues");
        sync.pump().await.expect("metadata imports");
    }
    fn temporary_files(&self) -> Vec<PathBuf> {
        fn walk(path: &Path, output: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(path).expect("data directory") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    walk(&path, output);
                } else if path.extension().is_some_and(|extension| extension == "tmp") {
                    output.push(path);
                }
            }
        }
        let mut output = Vec::new();
        walk(&self.target_root.join(".data"), &mut output);
        output
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.source_root).expect("source cleanup");
        fs::remove_dir_all(&self.target_root).expect("target cleanup");
    }
}
async fn poll_until_staged<F: Future>(future: &mut Pin<Box<F>>) {
    poll_fn(|cx| {
        assert!(
            future.as_mut().poll(cx).is_pending(),
            "fetch must be waiting on the second chunk"
        );
        Poll::Ready(())
    })
    .await;
}

#[tokio::test]
async fn cancelling_a_polled_multichunk_fetch_cleans_staging_and_retries() {
    let fixture = Fixture::new(vec![
        Chunk::Bytes(Bytes::from_static(b"partial")),
        Chunk::Pending,
    ])
    .await;
    let control = OperationControl::default();
    let mut sync = fixture
        .target
        .metadata_sync(fixture.exchange.clone())
        .with_operation_control(control.clone());
    fixture.metadata(&mut sync).await;
    let mut fetch = Box::pin(sync.fetch("file"));
    poll_until_staged(&mut fetch).await;
    let stages = fixture.temporary_files();
    assert_eq!(stages.len(), 1);
    assert_eq!(fs::read(&stages[0]).expect("staged bytes"), b"partial");
    control.cancel();
    assert!(matches!(fetch.await, Err(Error::Cancelled)));
    assert_eq!(fixture.temporary_files(), [] as [PathBuf; 0]);
    assert!(matches!(
        fixture.target.read("file").await,
        Err(Error::ContentUnavailable)
    ));
    assert_eq!(*fixture.exchange.dropped.lock().expect("handle counter"), 1);
    *fixture.exchange.chunks.lock().expect("retry script") =
        vec![Chunk::Bytes(Bytes::from_static(b"complete")), Chunk::End].into();
    let retry = fixture.target.metadata_sync(fixture.exchange.clone());
    retry.fetch("file").await.expect("retry completes");
    assert_eq!(
        fixture.target.read("file").await.expect("complete visible"),
        b"complete"
    );
    assert_eq!(fixture.temporary_files(), [] as [PathBuf; 0]);
}

#[tokio::test]
async fn dropping_a_polled_fetch_and_stream_errors_remove_only_abandoned_staging() {
    for interrupted in [true, false] {
        let fixture = Fixture::new(vec![
            Chunk::Bytes(Bytes::from_static(b"partial")),
            if interrupted {
                Chunk::Pending
            } else {
                Chunk::Error
            },
        ])
        .await;
        let mut sync = fixture.target.metadata_sync(fixture.exchange.clone());
        fixture.metadata(&mut sync).await;
        fixture
            .target
            .write("retained", b"earlier complete file")
            .await
            .expect("earlier local commit");
        if interrupted {
            let mut fetch = Box::pin(sync.fetch("file"));
            poll_until_staged(&mut fetch).await;
            assert_eq!(fixture.temporary_files().len(), 1);
            drop(fetch);
        } else {
            assert!(matches!(sync.fetch("file").await, Err(Error::Offline)));
        }
        assert_eq!(fixture.temporary_files(), [] as [PathBuf; 0]);
        assert!(matches!(
            fixture.target.read("file").await,
            Err(Error::ContentUnavailable)
        ));
        assert_eq!(
            fixture
                .target
                .read("retained")
                .await
                .expect("earlier commit intact"),
            b"earlier complete file"
        );
        assert_eq!(*fixture.exchange.dropped.lock().expect("handle counter"), 1);
    }
}

#[tokio::test]
async fn revision_change_during_staging_cannot_replace_newer_valid_content() {
    let release = Arc::new(Notify::new());
    let fixture = Fixture::new(vec![
        Chunk::Bytes(Bytes::from_static(b"old transfer")),
        Chunk::Wait(Arc::clone(&release)),
    ])
    .await;
    let mut sync = fixture.target.metadata_sync(fixture.exchange.clone());
    fixture.metadata(&mut sync).await;
    let mut fetch = Box::pin(sync.fetch("file"));
    poll_until_staged(&mut fetch).await;
    let newer = fixture
        .target
        .write("file", b"newer local content")
        .await
        .expect("newer local revision");
    release.notify_one();
    assert!(matches!(fetch.await, Err(Error::StaleRevision)));
    assert_eq!(fixture.temporary_files(), [] as [PathBuf; 0]);
    assert_eq!(
        fixture
            .target
            .read("file")
            .await
            .expect("newer content intact"),
        b"newer local content"
    );
    assert_eq!(
        fixture
            .target
            .revision("file")
            .await
            .expect("newer revision intact"),
        newer.meta.revision
    );
}

#[tokio::test]
async fn complete_staging_still_requires_a_new_publication_decision() {
    let release = Arc::new(Notify::new());
    let fixture = Fixture::new(vec![
        Chunk::Bytes(Bytes::from_static(b"complete")),
        Chunk::Wait(Arc::clone(&release)),
    ])
    .await;
    let allowed = Arc::new(AtomicBool::new(true));
    let policy = Arc::clone(&allowed);
    let control = OperationControl::new(move |_, _| {
        let policy = Arc::clone(&policy);
        Box::pin(async move { policy.load(Ordering::SeqCst) })
    });
    let mut sync = fixture
        .target
        .metadata_sync(fixture.exchange.clone())
        .with_operation_control(control);
    fixture.metadata(&mut sync).await;
    let mut fetch = Box::pin(sync.fetch("file"));
    poll_until_staged(&mut fetch).await;
    allowed.store(false, Ordering::SeqCst);
    release.notify_one();
    assert!(matches!(fetch.await, Err(Error::PermissionDenied)));
    assert_eq!(fixture.temporary_files(), [] as [PathBuf; 0]);
    assert!(matches!(
        fixture.target.read("file").await,
        Err(Error::ContentUnavailable)
    ));
}

#[tokio::test]
async fn imported_new_revision_needs_new_content_and_keeps_earlier_complete_bytes() {
    let fixture = Fixture::new(vec![
        Chunk::Bytes(Bytes::from_static(b"complete")),
        Chunk::End,
    ])
    .await;
    let mut sync = fixture.target.metadata_sync(fixture.exchange.clone());
    fixture.metadata(&mut sync).await;
    sync.fetch("file").await.expect("first complete fetch");
    let previous = fixture
        .target
        .entry("file")
        .await
        .expect("previous revision");
    fixture
        .source
        .write("file", b"replacement")
        .await
        .expect("new source revision");
    fixture.metadata(&mut sync).await;
    assert!(matches!(
        fixture.target.read("file").await,
        Err(Error::ContentUnavailable)
    ));
    *fixture.exchange.chunks.lock().expect("script") =
        vec![Chunk::Bytes(Bytes::from_static(b"partial")), Chunk::Error].into();
    assert!(matches!(sync.fetch("file").await, Err(Error::Offline)));
    assert_eq!(
        fs::read(
            fixture
                .target_root
                .join(".data")
                .join(previous.meta.file_id.to_string())
                .join(previous.meta.revision.to_string())
        )
        .expect("earlier complete version retained"),
        b"complete"
    );
    *fixture.exchange.chunks.lock().expect("script") =
        vec![Chunk::Bytes(Bytes::from_static(b"replacement")), Chunk::End].into();
    sync.fetch("file").await.expect("new revision retry");
    assert_eq!(
        fixture
            .target
            .read("file")
            .await
            .expect("new content visible"),
        b"replacement"
    );
    assert_eq!(fixture.temporary_files(), [] as [PathBuf; 0]);
}
