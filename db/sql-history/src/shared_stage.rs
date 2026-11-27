use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use std::{
    io,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    thread,
};

use futures::executor::block_on;

use btree::{BTree, BTreeRead, BTreeTransaction};
use btree_redb::SharedNamespace;

use crate::{FileSyncStage, FileSyncStageFactory, SyncStage, SyncStageFactory};

const MAX_SHARED_STAGES: usize = 128;
static ACTIVE_SHARED_STAGES: AtomicUsize = AtomicUsize::new(0);
static CLEANUP: OnceLock<io::Result<SyncSender<Cleanup>>> = OnceLock::new();

#[derive(Debug)]
struct StageSlot;

impl StageSlot {
    fn reserve() -> io::Result<Self> {
        cleanup_sender()?;
        let mut active = ACTIVE_SHARED_STAGES.load(Ordering::Relaxed);
        loop {
            if active >= MAX_SHARED_STAGES {
                return Err(io::Error::other("shared SQL stage limit reached"));
            }
            match ACTIVE_SHARED_STAGES.compare_exchange_weak(
                active,
                active + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => active = current,
            }
        }
        Ok(Self)
    }
}

impl Drop for StageSlot {
    fn drop(&mut self) {
        ACTIVE_SHARED_STAGES.fetch_sub(1, Ordering::Relaxed);
    }
}

struct Cleanup {
    namespace: SharedNamespace,
    table: String,
    _slot: StageSlot,
}

fn cleanup_sender() -> io::Result<&'static SyncSender<Cleanup>> {
    CLEANUP
        .get_or_init(|| {
            let (send, receive) = sync_channel::<Cleanup>(MAX_SHARED_STAGES);
            thread::Builder::new()
                .name("ofdb-sql-stage-cleanup".into())
                .spawn(move || {
                    for job in receive {
                        let _ = block_on(job.namespace.remove_table(&job.table));
                    }
                })?;
            Ok(send)
        })
        .as_ref()
        .map_err(|error| io::Error::new(error.kind(), error.to_string()))
}

#[derive(Debug)]
pub enum NativeSyncStageFactory {
    File(FileSyncStageFactory),
    Shared(SharedNamespace),
}

impl From<FileSyncStageFactory> for NativeSyncStageFactory {
    fn from(factory: FileSyncStageFactory) -> Self {
        Self::File(factory)
    }
}

impl NativeSyncStageFactory {
    #[must_use]
    pub const fn shared(namespace: SharedNamespace) -> Self {
        Self::Shared(namespace)
    }
}

#[derive(Debug)]
pub enum NativeSyncStage {
    File(FileSyncStage),
    Shared(SharedSyncStage),
}

impl SyncStageFactory for NativeSyncStageFactory {
    type Stage = NativeSyncStage;
    type Error = io::Error;

    async fn create(&self) -> io::Result<Self::Stage> {
        match self {
            Self::File(factory) => factory.create().await.map(NativeSyncStage::File),
            Self::Shared(namespace) => {
                namespace.check()?;
                Ok(NativeSyncStage::Shared(SharedSyncStage {
                    namespace: namespace.clone(),
                    table: format!("stage:{}", uuid::Uuid::now_v7()),
                    count: 0,
                    next: 0,
                    finished: false,
                    slot: Some(StageSlot::reserve()?),
                }))
            }
        }
    }
}

#[derive(Debug)]
pub struct SharedSyncStage {
    namespace: SharedNamespace,
    table: String,
    count: u64,
    next: u64,
    finished: bool,
    slot: Option<StageSlot>,
}

impl SyncStage for NativeSyncStage {
    type Error = io::Error;

    #[allow(
        clippy::significant_drop_tightening,
        reason = "commit consumes the stage transaction"
    )]
    async fn append(&mut self, record: Vec<u8>) -> io::Result<()> {
        match self {
            Self::File(stage) => stage.append(record).await,
            Self::Shared(stage) => {
                if stage.finished {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "sync stage is finished",
                    ));
                }
                stage.namespace.check()?;
                let tree = stage.namespace.tree(&stage.table);
                let mut transaction = tree.transaction().await.map_err(io::Error::other)?;
                transaction
                    .insert(stage.count.to_be_bytes().to_vec(), record)
                    .await
                    .map_err(io::Error::other)?;
                transaction.commit().await.map_err(io::Error::other)?;
                stage.count = stage
                    .count
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("stage record count overflow"))?;
                Ok(())
            }
        }
    }

    async fn finish(&mut self) -> io::Result<()> {
        match self {
            Self::File(stage) => stage.finish().await,
            Self::Shared(stage) => {
                stage.finished = true;
                Ok(())
            }
        }
    }

    async fn next_record(&mut self) -> io::Result<Option<Vec<u8>>> {
        match self {
            Self::File(stage) => stage.next_record().await,
            Self::Shared(stage) => {
                if !stage.finished {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "sync stage is not finished",
                    ));
                }
                if stage.next == stage.count {
                    return Ok(None);
                }
                stage.namespace.check()?;
                let record = stage
                    .namespace
                    .tree(&stage.table)
                    .get(&stage.next.to_be_bytes().to_vec())
                    .await
                    .map_err(io::Error::other)?
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "missing sync stage record")
                    })?;
                stage.next += 1;
                Ok(Some(record))
            }
        }
    }
}

impl Drop for SharedSyncStage {
    fn drop(&mut self) {
        if self.count != 0
            && let Ok(false) = self.namespace.try_remove_table(&self.table)
        {
            // Each live stage or queued cleanup owns a slot, so cleanup cannot fill the queue.
            let sender = cleanup_sender().expect("stage creation initialized the cleanup worker");
            let job = Cleanup {
                namespace: self.namespace.clone(),
                table: core::mem::take(&mut self.table),
                _slot: self.slot.take().expect("stage owns one cleanup slot"),
            };
            let _ = sender.try_send(job);
        }
    }
}
