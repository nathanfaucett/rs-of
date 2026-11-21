use std::{
    collections::BTreeMap,
    fmt::{self, Debug, Display},
    io,
};

use futures_util::stream::try_unfold;
use ofdb_kv::KvSnapshot;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::sync::{broadcast, mpsc::error::TrySendError};

use ofdb_platform::{MaybeSend, MaybeSync};

use crate::{
    Error, FileHandle, FileHandleId, FileOperation, FileResponse, FileServiceError,
    FileSessionService, FileSystem, IncomingMessage, MAX_FILE_READ_LENGTH, OpenRequest,
    OperationControl, RemoteOperation, Residency, SessionRequest,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SyncMessage<PeerId> {
    SnapshotRequest { peer: PeerId },
    SnapshotDelta { snapshots: Vec<KvSnapshot> },
}

pub struct MetadataSync<'a, PeerId, T>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + MaybeSend + MaybeSync + 'static,
    T: FileSessionService<PeerId> + MaybeSync + Clone + MaybeSend + 'static,
{
    file_system: &'a FileSystem<PeerId>,
    transport: T,
    incoming: broadcast::Receiver<IncomingMessage<PeerId>>,
    sessions: broadcast::Receiver<SessionRequest<PeerId>>,
    changes: broadcast::Receiver<()>,
    handles: BTreeMap<(PeerId, FileHandleId), crate::LocalFileHandle<PeerId>>,
    last_snapshots: Vec<KvSnapshot>,
    control: OperationControl<PeerId>,
}

impl<PeerId, T> fmt::Debug for MetadataSync<'_, PeerId, T>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + MaybeSend + MaybeSync + 'static,
    T: FileSessionService<PeerId> + MaybeSync + Clone + MaybeSend + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MetadataSync")
            .field("handles", &self.handles.len())
            .field("last_snapshots", &self.last_snapshots.len())
            .finish_non_exhaustive()
    }
}

impl<PeerId> FileSystem<PeerId>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + MaybeSend + MaybeSync + 'static,
{
    pub fn metadata_sync<T>(&self, transport: T) -> MetadataSync<'_, PeerId, T>
    where
        T: FileSessionService<PeerId> + MaybeSync + Clone + MaybeSend + 'static,
    {
        let incoming = transport.subscribe();
        let sessions = transport.subscribe_file_sessions();
        MetadataSync {
            file_system: self,
            transport,
            incoming,
            sessions,
            changes: self.changes.subscribe(),
            handles: BTreeMap::new(),
            last_snapshots: Vec::new(),
            control: OperationControl::default(),
        }
    }

    /// Synchronizes metadata and file sessions with all transport peers.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, storage, admission, or transport failures.
    pub async fn sync_peer<T>(&self, transport: T) -> Result<(), Error>
    where
        T: FileSessionService<PeerId> + MaybeSync + Clone + MaybeSend + 'static,
    {
        self.metadata_sync(transport).run().await
    }
}

impl<PeerId, T> MetadataSync<'_, PeerId, T>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + MaybeSend + MaybeSync + 'static,
    T: FileSessionService<PeerId> + MaybeSync + Clone + MaybeSend + 'static,
{
    async fn wait<F: Future>(&self, future: F) -> Result<F::Output, Error> {
        self.file_system
            .owner
            .wait(self.control.wait(future))
            .await?
    }

    async fn admit(&self, peer: PeerId, operation: RemoteOperation) -> Result<(), Error> {
        self.file_system
            .owner
            .wait(self.control.admit(peer, operation))
            .await?
    }

    #[must_use]
    pub fn with_operation_control(mut self, control: OperationControl<PeerId>) -> Self {
        self.control = control;
        self
    }

    /// Announces the current metadata snapshot to all peers.
    ///
    /// # Errors
    ///
    /// Returns an error for snapshot export, admission, or transport failures.
    pub async fn announce(&self) -> Result<(), Error> {
        let snapshots = self.export_snapshots().await?;
        for peer in self.transport.peers() {
            self.send_metadata(
                peer.clone(),
                SyncMessage::SnapshotRequest { peer: peer.clone() },
            )
            .await?;
            self.send_metadata(
                peer,
                SyncMessage::SnapshotDelta {
                    snapshots: snapshots.clone(),
                },
            )
            .await?;
        }
        Ok(())
    }

    /// Fetches one file revision from a reachable provider when absent locally.
    ///
    /// # Errors
    ///
    /// Returns an error for missing content/providers, admission, transport, or storage failures.
    pub async fn fetch(&self, path: &str) -> Result<(), Error> {
        let entry = self.file_system.entry(path).await?;
        if entry.meta.kind == crate::FileKind::Directory {
            return Err(Error::IsDirectory);
        }
        let revision = self.file_system.revision(path).await?;
        if self
            .file_system
            .content(entry.meta.file_id, revision)
            .await
            .is_ok()
        {
            return Ok(());
        }
        let provider = self.provider(&entry.meta)?;
        let handle = self
            .wait(self.transport.open(
                provider.clone(),
                OpenRequest {
                    path: path.to_owned(),
                    revision: Some(revision),
                },
            ))
            .await?
            .map_err(|_error| Error::Offline)?;
        let content = try_unfold((handle, 0_u64), |(mut handle, offset)| async move {
            let bytes = handle
                .read(offset, MAX_FILE_READ_LENGTH)
                .await
                .map_err(|_error| Error::Offline)?;
            if bytes.is_empty() {
                Box::new(handle)
                    .close()
                    .await
                    .map_err(|_error| Error::Offline)?;
                return Ok(None);
            }
            let next_offset = offset.checked_add(bytes.len() as u64).ok_or_else(|| {
                Error::Io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "file offset overflow during sync",
                ))
            })?;
            Ok(Some((bytes, (handle, next_offset))))
        });
        self.file_system
            .store_content(
                entry.meta.file_id,
                revision,
                Box::pin(content),
                &self.control,
                provider,
            )
            .await
    }

    /// Fetches content when needed and changes a path residency rule.
    ///
    /// # Errors
    ///
    /// Returns an error for fetch, admission, or storage failures.
    pub async fn set_residency(&self, path: &str, residency: Residency) -> Result<(), Error> {
        if residency == Residency::Full {
            self.fetch(path).await?;
        }
        self.file_system.set_residency(path, residency).await
    }

    /// Runs synchronization until cancellation or channel closure.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, retirement, storage, admission, or transport failures.
    pub async fn run(mut self) -> Result<(), Error> {
        self.announce().await?;
        self.last_snapshots = self.export_snapshots().await?;
        loop {
            tokio::select! {
                biased;
                () = self.control.cancelled() => return Err(Error::Cancelled),
                () = self.file_system.owner.retired() => return Err(Error::PermissionDenied),
                result = self.changes.recv() => match result {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => self.publish_changes().await?,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                },
                result = self.incoming.recv() => match result {
                    Ok((peer, message)) => self.receive(peer, message).await?,
                    Err(broadcast::error::RecvError::Lagged(_)) => self.announce().await?,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                },
                result = self.sessions.recv() => match result {
                    Ok((peer, operation, response)) => self.receive_session(peer, operation, response).await?,
                    Err(broadcast::error::RecvError::Lagged(_)) => self.announce().await?,
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                },
            }
        }
    }

    /// Processes currently pending synchronization work.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, retirement, storage, admission, or transport failures.
    pub async fn pump(&mut self) -> Result<(), Error> {
        let result = self.pump_inner().await;
        if self.control.is_cancelled() || self.file_system.owner.check().is_err() {
            self.handles.clear();
            while self.sessions.try_recv().is_ok() {}
            while self.incoming.try_recv().is_ok() {}
        }
        result
    }

    async fn pump_inner(&mut self) -> Result<(), Error> {
        if self.control.is_cancelled() || self.file_system.owner.check().is_err() {
            self.handles.clear();
            while self.sessions.try_recv().is_ok() {}
            while self.incoming.try_recv().is_ok() {}
            self.file_system.owner.check()?;
            return Err(Error::Cancelled);
        }
        while self.changes.try_recv().is_ok() {}
        self.publish_changes().await?;
        loop {
            match self.incoming.try_recv() {
                Ok((peer, message)) => self.receive(peer, message).await?,
                Err(broadcast::error::TryRecvError::Lagged(_)) => {
                    self.announce().await?;
                }
                Err(
                    broadcast::error::TryRecvError::Empty | broadcast::error::TryRecvError::Closed,
                ) => break,
            }
        }
        while let Ok((peer, operation, response)) = self.sessions.try_recv() {
            self.receive_session(peer, operation, response).await?;
        }
        Ok(())
    }

    async fn export_snapshots(&self) -> Result<Vec<KvSnapshot>, Error> {
        self.file_system
            .access()
            .await?
            .database()
            .export_snapshots()
            .await
            .map_err(metadata_error)
    }

    async fn publish_changes(&mut self) -> Result<(), Error> {
        let snapshots = self.export_snapshots().await?;
        if snapshots != self.last_snapshots {
            for peer in self.transport.peers() {
                self.send_metadata(
                    peer,
                    SyncMessage::SnapshotDelta {
                        snapshots: snapshots.clone(),
                    },
                )
                .await?;
            }
            self.last_snapshots = snapshots;
        }
        Ok(())
    }

    async fn send_metadata(&self, peer: PeerId, message: SyncMessage<PeerId>) -> Result<(), Error> {
        match self
            .admit(peer.clone(), RemoteOperation::ExportSnapshots)
            .await
        {
            Ok(()) => {}
            Err(Error::PermissionDenied) => {
                self.file_system.owner.check()?;
                return Ok(());
            }
            Err(error) => return Err(error),
        }
        self.wait(self.transport.send(peer, message))
            .await?
            .map_err(transport_error)
    }

    fn provider(&self, meta: &crate::FileMeta<PeerId>) -> Result<PeerId, Error> {
        let peers = self.transport.peers();
        if peers.is_empty() {
            return Err(Error::NoPeers);
        }
        meta.providers
            .iter()
            .find(|provider| peers.contains(provider))
            .cloned()
            .ok_or(Error::NoReachableProvider)
    }

    async fn receive(&mut self, peer: PeerId, message: SyncMessage<PeerId>) -> Result<(), Error> {
        match message {
            SyncMessage::SnapshotRequest { .. } => {
                let snapshots = self.export_snapshots().await?;
                self.send_metadata(peer, SyncMessage::SnapshotDelta { snapshots })
                    .await
            }
            SyncMessage::SnapshotDelta { snapshots } => {
                let _execution = self.wait(self.file_system.owner.execution.lock()).await?;
                match self.admit(peer, RemoteOperation::ImportSnapshots).await {
                    Ok(()) => {}
                    Err(Error::PermissionDenied) => return Ok(()),
                    Err(error) => return Err(error),
                }
                let access = self.file_system.access().await?;
                access
                    .database()
                    .import_snapshots(snapshots)
                    .await
                    .map_err(metadata_error)?;
                self.last_snapshots = access
                    .database()
                    .export_snapshots()
                    .await
                    .map_err(metadata_error)?;
                drop(access);
                Ok(())
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "session admission, execution, and response share one cancellation boundary"
    )]
    async fn receive_session(
        &mut self,
        peer: PeerId,
        operation: FileOperation,
        response: crate::SessionResponse<PeerId>,
    ) -> Result<(), Error> {
        let result: Result<Vec<FileResponse<PeerId>>, Error> = async {
            let execution = self.wait(self.file_system.owner.execution.lock()).await?;
            let path = match &operation {
                FileOperation::Open(request) => Some(request.path.clone()),
                FileOperation::Read { handle, .. }
                | FileOperation::Write { handle, .. }
                | FileOperation::Scan { handle, .. }
                | FileOperation::Close { handle } => self
                    .handles
                    .get(&(peer.clone(), handle.clone()))
                    .map(|state| state.path.clone()),
            };
            self.admit(
                peer.clone(),
                RemoteOperation::Session {
                    operation: operation.clone(),
                    path,
                },
            )
            .await?;
            let access = self.file_system.access().await?;
            let _execution = if matches!(operation, FileOperation::Write { .. }) {
                Some(execution)
            } else {
                drop(execution);
                None
            };
            match operation {
                FileOperation::Open(request) => {
                    let entry = access.entry(&request.path).await?;
                    let revision = access.revision(&request.path).await?;
                    if request
                        .revision
                        .is_some_and(|expected| expected != revision)
                    {
                        Err(Error::StaleRevision)
                    } else {
                        let id = self.transport.allocate_handle(&peer, &request);
                        let handle = access.open_handle(request).await?;
                        self.handles.insert((peer.clone(), id.clone()), handle);
                        Ok(vec![FileResponse::Opened {
                            handle: id,
                            entry,
                            revision,
                        }])
                    }
                }
                FileOperation::Read {
                    handle,
                    offset,
                    length,
                } => {
                    if length > MAX_FILE_READ_LENGTH {
                        return Err(Error::InvalidReadLength);
                    }
                    let handle = self
                        .handles
                        .get_mut(&(peer.clone(), handle))
                        .ok_or(Error::NotFound)?;
                    let bytes = handle.read_admitted(&access, offset, length).await?;
                    Ok(vec![FileResponse::ReadChunk(bytes), FileResponse::ReadEnd])
                }
                FileOperation::Write {
                    handle,
                    expected_revision,
                    offset,
                    data,
                } => {
                    let state = self
                        .handles
                        .get_mut(&(peer.clone(), handle))
                        .ok_or(Error::NotFound)?;
                    if state.revision == expected_revision {
                        let count = access
                            .write_at_inner(&state.path, expected_revision, offset, data)
                            .await?;
                        state.revision = access.revision(&state.path).await?;
                        Ok(vec![FileResponse::Written {
                            count,
                            revision: state.revision,
                        }])
                    } else {
                        Err(Error::StaleRevision)
                    }
                }
                FileOperation::Scan {
                    handle,
                    cursor,
                    limit,
                } => {
                    let state = self
                        .handles
                        .get_mut(&(peer.clone(), handle))
                        .ok_or(Error::NotFound)?;
                    Ok(vec![FileResponse::Page(
                        access.scan(&state.path, cursor.as_deref(), limit).await?,
                    )])
                }
                FileOperation::Close { handle } => {
                    let state = self
                        .handles
                        .remove(&(peer, handle))
                        .ok_or(Error::NotFound)?;
                    state.close().await?;
                    Ok(vec![FileResponse::Closed])
                }
            }
        }
        .await;
        if self.control.is_cancelled() || self.file_system.owner.check().is_err() {
            self.handles.clear();
        }
        let responses = result
            .map_err(|error| service_error(&error))
            .unwrap_or_else(|error| vec![FileResponse::Error(error)]);
        for item in responses {
            if let Err(TrySendError::Full(item)) = response.try_send(item) {
                let _ = self.wait(response.send(item)).await;
            }
        }
        if self.control.is_cancelled() || self.file_system.owner.check().is_err() {
            self.handles.clear();
        }
        Ok(())
    }
}

fn metadata_error(error: impl Display) -> Error {
    Error::Metadata(error.to_string())
}

fn transport_error(error: impl Display) -> Error {
    Error::Metadata(format!("transport error: {error}"))
}

const fn service_error(error: &Error) -> FileServiceError {
    match error {
        Error::NotFound => FileServiceError::NotFound,
        Error::NotDirectory => FileServiceError::NotDirectory,
        Error::IsDirectory => FileServiceError::IsDirectory,
        Error::InvalidReadLength => FileServiceError::InvalidReadLength,
        Error::InvalidScanCursor => FileServiceError::InvalidScanCursor,
        Error::InvalidScanLimit => FileServiceError::InvalidScanLimit,
        Error::StaleRevision => FileServiceError::StaleRevision,
        Error::PassthroughWrite => FileServiceError::PassthroughWrite,
        Error::ContentUnavailable => FileServiceError::ContentUnavailable,
        Error::PermissionDenied | Error::Cancelled => FileServiceError::PermissionDenied,
        _ => FileServiceError::Io,
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use bytes::Bytes;
    use tokio::sync::mpsc;

    use super::*;
    use crate::MemoryNetwork;

    async fn request(
        sync: &mut MetadataSync<'_, u8, crate::MemoryTransport<u8>>,
        peer: u8,
        operation: FileOperation,
    ) -> Vec<FileResponse<u8>> {
        let (sender, mut receiver) = mpsc::channel(4);
        sync.receive_session(peer, operation, sender)
            .await
            .expect("session request should complete");
        let mut responses = Vec::new();
        while let Some(response) = receiver.recv().await {
            responses.push(response);
        }
        responses
    }

    #[tokio::test]
    async fn rejects_oversized_file_read_before_allocating() {
        let root = env::temp_dir().join(format!("file-system-{}", uuid::Uuid::now_v7()));
        let file_system = FileSystem::open(&root, 1_u8)
            .await
            .expect("file system should open");
        file_system
            .set_residency("", Residency::Full)
            .await
            .expect("full residency should be set");
        file_system
            .write("file.txt", b"ok")
            .await
            .expect("file should be written");
        let mut sync = file_system.metadata_sync(MemoryNetwork::new(16).transport(1));
        let opened = request(
            &mut sync,
            2,
            FileOperation::Open(OpenRequest {
                path: "file.txt".to_owned(),
                revision: None,
            }),
        )
        .await;
        let [FileResponse::Opened { handle, .. }] = opened.as_slice() else {
            panic!("file should be opened");
        };
        let handle = handle.clone();
        assert_eq!(
            request(
                &mut sync,
                2,
                FileOperation::Read {
                    handle: handle.clone(),
                    offset: 0,
                    length: MAX_FILE_READ_LENGTH + 1,
                },
            )
            .await,
            [FileResponse::Error(FileServiceError::InvalidReadLength)]
        );
        assert_eq!(
            request(
                &mut sync,
                2,
                FileOperation::Read {
                    handle,
                    offset: 0,
                    length: 2,
                },
            )
            .await,
            [
                FileResponse::ReadChunk(Bytes::from_static(b"ok")),
                FileResponse::ReadEnd
            ]
        );
        fs::remove_dir_all(root).expect("test directory should be removed");
    }

    #[tokio::test]
    #[allow(
        clippy::too_many_lines,
        reason = "peer-scoped lifecycle fixture covers every session operation"
    )]
    async fn file_handles_are_scoped_to_requesting_peer() {
        let root = env::temp_dir().join(format!("file-system-{}", uuid::Uuid::now_v7()));
        let file_system = FileSystem::open(&root, 1_u8)
            .await
            .expect("file system should open");
        file_system
            .set_residency("", Residency::Full)
            .await
            .expect("full residency should be set");
        file_system
            .write("file.txt", b"original")
            .await
            .expect("file should be written");
        file_system
            .create_dir("dir")
            .await
            .expect("directory should be created");
        let network = MemoryNetwork::new(16);
        let mut sync = file_system.metadata_sync(network.transport(1));

        for path in ["file.txt", "dir"] {
            let opened = request(
                &mut sync,
                2,
                FileOperation::Open(OpenRequest {
                    path: path.to_owned(),
                    revision: None,
                }),
            )
            .await;
            let [
                FileResponse::Opened {
                    handle, revision, ..
                },
            ] = opened.as_slice()
            else {
                panic!("owner should open {path}");
            };
            let handle = handle.clone();
            let revision = *revision;

            let attempts = if path == "file.txt" {
                vec![
                    FileOperation::Read {
                        handle: handle.clone(),
                        offset: 0,
                        length: 8,
                    },
                    FileOperation::Write {
                        handle: handle.clone(),
                        expected_revision: revision,
                        offset: 0,
                        data: Bytes::from_static(b"intruder"),
                    },
                ]
            } else {
                vec![FileOperation::Scan {
                    handle: handle.clone(),
                    cursor: None,
                    limit: 1,
                }]
            };
            for operation in attempts.into_iter().chain([FileOperation::Close {
                handle: handle.clone(),
            }]) {
                assert_eq!(
                    request(&mut sync, 3, operation).await,
                    [FileResponse::Error(FileServiceError::NotFound)]
                );
            }
            let own_operation = if path == "file.txt" {
                FileOperation::Read {
                    handle: handle.clone(),
                    offset: 0,
                    length: 8,
                }
            } else {
                FileOperation::Scan {
                    handle: handle.clone(),
                    cursor: None,
                    limit: 1,
                }
            };
            assert!(!matches!(
                request(&mut sync, 2, own_operation).await.as_slice(),
                [FileResponse::Error(_)]
            ));

            let other = request(
                &mut sync,
                3,
                FileOperation::Open(OpenRequest {
                    path: path.to_owned(),
                    revision: None,
                }),
            )
            .await;
            let [
                FileResponse::Opened {
                    handle: other_handle,
                    ..
                },
            ] = other.as_slice()
            else {
                panic!("second peer should open {path}");
            };
            assert_eq!(&handle, other_handle);
            assert_eq!(
                request(
                    &mut sync,
                    3,
                    FileOperation::Close {
                        handle: handle.clone()
                    }
                )
                .await,
                [FileResponse::Closed]
            );
            assert_eq!(
                request(&mut sync, 2, FileOperation::Close { handle }).await,
                [FileResponse::Closed]
            );
        }
        assert_eq!(
            file_system
                .read("file.txt")
                .await
                .expect("file should remain readable"),
            b"original"
        );
        fs::remove_dir_all(root).expect("test directory should be removed");
    }
}
