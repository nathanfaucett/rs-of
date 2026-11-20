#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::{
    fmt::{self, Debug, Display},
    io,
    ops::Deref,
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::sync::{RwLockReadGuard, broadcast};
use uuid::Uuid;

use ofdb_kv::{Client, Database, Value};
use ofdb_platform::{MaybeSend, MaybeSync};

use crate::{
    Error, FileKind, FileMeta, OperationControl, RemoteOperation, Residency,
    content::{ContentObject, StagedObject},
    file_service::{LocalFileHandle, ScanPage, scan_page},
    managed_copy::{ManagedCopy, Storage},
    path::{directory, file},
    residency::ResidencyRules,
    stream::ReadStream,
};

#[cfg(test)]
use crate::staged_content::{ContentAction, ContentStep};
#[cfg(not(target_arch = "wasm32"))]
use crate::{native, shared::open as open_shared_storage};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(bound(deserialize = "PeerId: Ord + Deserialize<'de>"))]
pub struct Entry<PeerId = Uuid> {
    pub path: String,
    pub meta: FileMeta<PeerId>,
}

pub struct FileSystem<PeerId = Uuid>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + 'static,
{
    pub(crate) owner: Arc<ManagedCopy>,
    node_id: PeerId,
    residency: Arc<Mutex<ResidencyRules>>,
    pub(crate) changes: broadcast::Sender<()>,
    #[cfg(test)]
    pub(crate) content_fault: Mutex<Option<ContentStep>>,
    #[cfg(test)]
    pub(crate) content_action: Mutex<Option<ContentAction>>,
}

impl<PeerId> fmt::Debug for FileSystem<PeerId>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSystem")
            .field("node_id", &self.node_id)
            .finish_non_exhaustive()
    }
}

impl<PeerId> Clone for FileSystem<PeerId>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + 'static,
{
    fn clone(&self) -> Self {
        Self {
            owner: Arc::clone(&self.owner),
            node_id: self.node_id.clone(),
            residency: Arc::clone(&self.residency),
            changes: self.changes.clone(),
            #[cfg(test)]
            content_fault: Mutex::new(None),
            #[cfg(test)]
            content_action: Mutex::new(None),
        }
    }
}

impl<PeerId> FileSystem<PeerId>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + MaybeSend + MaybeSync + 'static,
{
    /// Opens a native filesystem root.
    ///
    /// # Errors
    ///
    /// Returns an error for storage, path, or lifecycle failures.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn open(root: impl AsRef<Path>, node_id: PeerId) -> Result<Self, Error> {
        let owner = native::open(root.as_ref()).await?;
        Ok(Self::new(owner, node_id))
    }

    /// Opens a filesystem in a shared database host.
    ///
    /// # Errors
    ///
    /// Returns an error for storage, persisted residency, or lifecycle failures.
    ///
    /// # Panics
    ///
    /// Panics if admitted shared storage or its local-state client is missing.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn open_shared(
        host: &ofdb_kv::SharedDatabase,
        name: &str,
        node_id: PeerId,
    ) -> Result<Self, Error> {
        let owner = open_shared_storage(host, name).await?;
        let mut filesystem = Self::new(Arc::clone(&owner), node_id);
        filesystem.residency = Arc::clone(&owner.local_residency);
        let execution = owner.execution.lock().await;
        let storage = owner.admit().await?;
        let state = storage
            .as_ref()
            .expect("admitted storage")
            .local_state
            .as_ref()
            .expect("shared local state");
        for (key, value) in state
            .scan_prefix("residency:")
            .await
            .map_err(metadata_error)?
        {
            let mode = match value {
                Value::Bool(true) => Residency::Full,
                Value::Bool(false) => Residency::Passthrough,
                _ => return Err(metadata_error("invalid persisted residency")),
            };
            filesystem.residency.lock().expect("residency lock").set(
                key.strip_prefix("residency:")
                    .expect("selected residency key"),
                mode,
            )?;
        }
        drop(storage);
        drop(execution);
        Ok(filesystem)
    }

    #[cfg(all(target_arch = "wasm32", feature = "browser"))]
    pub async fn open_browser(root: String, node_id: PeerId) -> Result<Self, Error> {
        let owner = crate::browser::open(root).await?;
        Ok(Self::new(owner, node_id))
    }

    #[cfg_attr(
        all(target_arch = "wasm32", not(feature = "browser")),
        allow(dead_code)
    )]
    #[cfg_attr(
        all(target_arch = "wasm32", not(feature = "browser")),
        allow(dead_code)
    )]
    fn new(owner: Arc<ManagedCopy>, node_id: PeerId) -> Self {
        let changes = broadcast::channel(16).0;

        Self {
            owner,
            node_id,
            residency: Arc::new(Mutex::new(ResidencyRules::default())),
            changes,
            #[cfg(test)]
            content_fault: Mutex::new(None),
            #[cfg(test)]
            content_action: Mutex::new(None),
        }
    }

    /// Returns the residency rule for a path.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid path or a retired filesystem.
    ///
    /// # Panics
    ///
    /// Panics if the residency lock is poisoned.
    pub fn residency(&self, path: &str) -> Result<Residency, Error> {
        self.owner.check()?;
        self.residency
            .lock()
            .expect("residency lock poisoned")
            .get(path)
    }

    pub(crate) async fn store_content<S>(
        &self,
        file_id: Uuid,
        revision: Uuid,
        stream: S,
        control: &OperationControl<PeerId>,
        peer: PeerId,
    ) -> Result<(), Error>
    where
        S: futures_core::Stream<Item = Result<Bytes, Error>>,
    {
        self.access()
            .await?
            .store_content(file_id, revision, stream, control, peer)
            .await
    }

    pub(crate) async fn access(&self) -> Result<FileSystemAccess<'_, PeerId>, Error> {
        self.owner.refresh().await?;
        Ok(FileSystemAccess {
            file_system: self,
            storage: self.owner.admit().await?,
        })
    }
    /// Changes a path residency rule and applies its content requirements.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid paths, unavailable content, or storage failures.
    pub async fn set_residency(&self, path: &str, residency: Residency) -> Result<(), Error> {
        self.access().await?.set_residency(path, residency).await
    }

    /// Opens a local handle for a path and optional expected revision.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing path, stale revision, or storage failure.
    pub async fn open_handle(
        &self,
        request: crate::OpenRequest,
    ) -> Result<crate::LocalFileHandle<PeerId>, Error> {
        self.access().await?.open_handle(request).await
    }

    /// Reads one path entry.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid or missing path or a storage failure.
    pub async fn entry(&self, path: &str) -> Result<Entry<PeerId>, Error> {
        self.access().await?.entry(path).await
    }

    /// Returns the locally retained content length without fetching remote content.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid path, wrong file type, unavailable content, or storage failure.
    pub async fn local_content_len(&self, path: &str) -> Result<u64, Error> {
        self.access().await?.local_content_len(path).await
    }

    /// Reads a complete file.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid path, wrong file type, or content/storage failure.
    pub async fn read(&self, path: &str) -> Result<Vec<u8>, Error> {
        self.access().await?.read(path).await
    }

    /// Opens chunked reads for a file.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid chunk size, wrong file type, or storage failure.
    pub async fn stream(&self, path: &str, chunk_size: usize) -> Result<ReadStream, Error> {
        self.access().await?.stream(path, chunk_size).await
    }

    /// Atomically creates an empty regular file if the path does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error for existing entries, invalid paths, residency restrictions, or
    /// content/storage failures.
    pub async fn create_file(&self, path: &str) -> Result<Entry<PeerId>, Error> {
        self.access().await?.create_file(path).await
    }

    /// Replaces a file with new content.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid paths, residency restrictions, or content/storage failures.
    pub async fn write(&self, path: &str, content: &[u8]) -> Result<Entry<PeerId>, Error> {
        self.access().await?.write(path, content).await
    }

    /// Writes bytes at an offset if the expected revision is current.
    ///
    /// # Errors
    ///
    /// Returns an error for stale revisions, residency restrictions, or content/storage failures.
    pub async fn write_at(
        &self,
        path: &str,
        expected_revision: Uuid,
        offset: u64,
        data: Bytes,
    ) -> Result<u32, Error> {
        self.access()
            .await?
            .write_at(path, expected_revision, offset, data)
            .await
    }

    /// Changes a file length if the expected revision is current.
    ///
    /// Extension fills new bytes with zeroes. The returned revision identifies the new content.
    ///
    /// # Errors
    ///
    /// Returns an error for stale revisions, wrong file types, residency restrictions, or
    /// content/storage failures.
    pub async fn truncate(
        &self,
        path: &str,
        expected_revision: Uuid,
        length: u64,
    ) -> Result<Uuid, Error> {
        self.access()
            .await?
            .truncate(path, expected_revision, length)
            .await
    }

    /// Appends content to a file.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid paths, residency restrictions, or content/storage failures.
    pub async fn append(&self, path: &str, content: &[u8]) -> Result<(), Error> {
        self.access().await?.append(path, content).await
    }

    /// Creates a directory and missing parent directories.
    ///
    /// # Errors
    ///
    /// Returns an error for path conflicts, residency restrictions, or storage failures.
    pub async fn create_dir(&self, path: &str) -> Result<Entry<PeerId>, Error> {
        self.access().await?.create_dir(path).await
    }

    /// Lists immediate directory children.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid path, wrong file type, or storage failure.
    pub async fn list(&self, path: &str) -> Result<Vec<Entry<PeerId>>, Error> {
        self.access().await?.list(path).await
    }

    /// Scans one page of directory entries.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid cursor/limit, wrong file type, or storage failure.
    pub async fn scan(
        &self,
        path: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<ScanPage<PeerId>, Error> {
        self.access().await?.scan(path, cursor, limit).await
    }

    /// Deletes a path.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing path, residency restriction, or storage failure.
    pub async fn delete(&self, path: &str) -> Result<(), Error> {
        self.access().await?.delete(path).await
    }

    /// Renames a file or directory tree.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid paths, conflicts, residency restrictions, or storage failures.
    pub async fn rename(&self, from: &str, to: &str) -> Result<(), Error> {
        self.access().await?.rename(from, to).await
    }

    /// Returns the current content revision for a path.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing path or storage failure.
    pub async fn revision(&self, path: &str) -> Result<Uuid, Error> {
        self.access().await?.revision(path).await
    }

    pub(crate) async fn content(&self, file_id: Uuid, revision: Uuid) -> Result<Vec<u8>, Error> {
        self.access().await?.content(file_id, revision).await
    }
}

pub(crate) struct FileSystemAccess<'a, PeerId>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + 'static,
{
    file_system: &'a FileSystem<PeerId>,
    storage: RwLockReadGuard<'a, Option<Storage>>,
}

impl<PeerId> Deref for FileSystemAccess<'_, PeerId>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + 'static,
{
    type Target = FileSystem<PeerId>;
    fn deref(&self) -> &Self::Target {
        self.file_system
    }
}

impl<PeerId> FileSystemAccess<'_, PeerId>
where
    PeerId: Clone + Debug + Ord + Serialize + DeserializeOwned + MaybeSend + MaybeSync + 'static,
{
    fn storage(&self) -> &Storage {
        self.storage.as_ref().expect("admitted storage")
    }
    pub(crate) fn database(&self) -> &Database {
        &self.storage().metadata
    }
    fn metadata(&self) -> &Client {
        &self.storage().client
    }
    fn content_store(&self) -> &Arc<dyn ContentObject> {
        &self.storage().content
    }
    pub(crate) fn residency(&self, path: &str) -> Result<Residency, Error> {
        self.file_system
            .residency
            .lock()
            .expect("residency lock")
            .get(path)
    }
    pub async fn set_residency(&self, path: &str, residency: Residency) -> Result<(), Error> {
        let _execution = self.owner.execution.lock().await;
        directory(path)?;
        if residency == Residency::Full {
            self.verify_content(path).await?;
        }
        if let Some(state) = &self.storage().local_state {
            state
                .set(
                    &format!("residency:{path}"),
                    Value::Bool(residency == Residency::Full),
                    None,
                )
                .await
                .map_err(metadata_error)?;
        }
        self.residency
            .lock()
            .expect("residency lock poisoned")
            .set(path, residency)?;
        if residency == Residency::Passthrough {
            self.evict_unreferenced(path).await?;
        }
        Ok(())
    }

    pub async fn open_handle(
        &self,
        request: crate::OpenRequest,
    ) -> Result<crate::LocalFileHandle<PeerId>, Error> {
        let entry = self.entry(&request.path).await?;
        let revision = self.revision(&request.path).await?;
        if request
            .revision
            .is_some_and(|expected| expected != revision)
        {
            return Err(Error::StaleRevision);
        }
        Ok(LocalFileHandle::new(
            self.file_system,
            request.path,
            revision,
            entry.meta.kind,
        ))
    }

    pub async fn entry(&self, path: &str) -> Result<Entry<PeerId>, Error> {
        directory(path)?;
        if path.is_empty() {
            return Err(Error::NotFound);
        }
        self.get(path)
            .await?
            .map(|meta| Entry {
                path: path.to_owned(),
                meta,
            })
            .ok_or(Error::NotFound)
    }

    pub async fn local_content_len(&self, path: &str) -> Result<u64, Error> {
        let _execution = self.owner.execution.lock().await;
        let entry = self.entry(path).await?;
        if entry.meta.kind == FileKind::Directory {
            return Err(Error::IsDirectory);
        }
        self.content_store()
            .content_len(entry.meta.file_id, entry.meta.revision)
            .await
    }

    pub async fn read(&self, path: &str) -> Result<Vec<u8>, Error> {
        let _execution = self.owner.execution.lock().await;
        let entry = self.entry(path).await?;
        if entry.meta.kind == FileKind::Directory {
            return Err(Error::IsDirectory);
        }
        self.content_store()
            .read(entry.meta.file_id, entry.meta.revision)
            .await
    }

    pub async fn stream(&self, path: &str, chunk_size: usize) -> Result<ReadStream, Error> {
        self.stream_revision(path, None, chunk_size, false).await
    }

    pub(crate) async fn stream_revision(
        &self,
        path: &str,
        expected_revision: Option<Uuid>,
        chunk_size: usize,
        admitted: bool,
    ) -> Result<ReadStream, Error> {
        let _execution = self.owner.execution.lock().await;
        let entry = self.entry(path).await?;
        if expected_revision.is_some_and(|revision| entry.meta.revision != revision) {
            return Err(Error::StaleRevision);
        }
        if entry.meta.kind == FileKind::Directory {
            return Err(Error::IsDirectory);
        }
        ReadStream::new(
            Arc::clone(self.content_store()),
            entry.meta.file_id,
            entry.meta.revision,
            chunk_size,
            Arc::clone(&self.owner),
            admitted,
        )
    }

    pub async fn create_file(&self, path: &str) -> Result<Entry<PeerId>, Error> {
        let _execution = self.owner.execution.lock().await;
        file(path)?;
        self.require_full(path)?;
        self.ensure_parent_directories(path).await?;
        if self.get(path).await?.is_some() {
            return Err(Error::AlreadyExists);
        }
        let meta = FileMeta::file(Uuid::now_v7(), self.node_id.clone());
        let mut stage = self.stage(meta.file_id, meta.revision).await?;
        self.stage_write(&mut *stage, &[]).await?;
        self.stage_sync(&mut *stage).await?;
        self.stage_publish(&mut *stage).await?;
        self.put(path, meta.clone()).await?;
        stage.commit();
        Ok(Entry {
            path: path.to_owned(),
            meta,
        })
    }

    pub async fn write(&self, path: &str, content: &[u8]) -> Result<Entry<PeerId>, Error> {
        let _execution = self.owner.execution.lock().await;
        file(path)?;
        self.require_full(path)?;
        self.ensure_parent_directories(path).await?;
        let mut meta = match self.get(path).await? {
            Some(meta) if meta.kind == FileKind::Directory => return Err(Error::IsDirectory),
            Some(meta) => meta,
            None => FileMeta::file(Uuid::now_v7(), self.node_id.clone()),
        };
        meta.revision = Uuid::now_v7();
        let mut stage = self.stage(meta.file_id, meta.revision).await?;
        self.stage_write(&mut *stage, content).await?;
        self.stage_sync(&mut *stage).await?;
        self.stage_publish(&mut *stage).await?;
        self.put(path, meta.clone()).await?;
        stage.commit();
        Ok(Entry {
            path: path.to_owned(),
            meta,
        })
    }

    pub async fn write_at(
        &self,
        path: &str,
        expected_revision: Uuid,
        offset: u64,
        data: Bytes,
    ) -> Result<u32, Error> {
        let _execution = self.owner.execution.lock().await;
        self.write_at_inner(path, expected_revision, offset, data)
            .await
    }

    pub(crate) async fn write_at_inner(
        &self,
        path: &str,
        expected_revision: Uuid,
        offset: u64,
        data: Bytes,
    ) -> Result<u32, Error> {
        file(path)?;
        self.require_full(path)?;
        let entry = self.entry(path).await?;
        if entry.meta.kind == FileKind::Directory {
            return Err(Error::IsDirectory);
        }
        if self.revision(path).await? != expected_revision {
            return Err(Error::StaleRevision);
        }
        let invalid_input =
            |message| Error::Io(io::Error::new(io::ErrorKind::InvalidInput, message));
        let data_len =
            u64::try_from(data.len()).map_err(|_error| invalid_input("write is too large"))?;
        let length =
            u32::try_from(data.len()).map_err(|_error| invalid_input("write is too large"))?;
        let end = offset
            .checked_add(data_len)
            .ok_or_else(|| invalid_input("write offset is too large"))?;
        usize::try_from(end).map_err(|_error| invalid_input("write offset is too large"))?;
        let mut meta = entry.meta.clone();
        meta.revision = Uuid::now_v7();
        let mut stage = self.stage(meta.file_id, meta.revision).await?;
        stage.copy_from(meta.file_id, entry.meta.revision).await?;
        self.stage_write_at(&mut *stage, offset, &data).await?;
        self.stage_sync(&mut *stage).await?;
        self.stage_publish(&mut *stage).await?;
        self.put(path, meta).await?;
        stage.commit();
        Ok(length)
    }

    pub async fn truncate(
        &self,
        path: &str,
        expected_revision: Uuid,
        length: u64,
    ) -> Result<Uuid, Error> {
        let _execution = self.owner.execution.lock().await;
        file(path)?;
        self.require_full(path)?;
        let entry = self.entry(path).await?;
        if entry.meta.kind == FileKind::Directory {
            return Err(Error::IsDirectory);
        }
        if entry.meta.revision != expected_revision {
            return Err(Error::StaleRevision);
        }
        usize::try_from(length).map_err(|_error| {
            Error::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "content length is too large",
            ))
        })?;
        let mut meta = entry.meta.clone();
        meta.revision = Uuid::now_v7();
        let revision = meta.revision;
        let mut stage = self.stage(meta.file_id, meta.revision).await?;
        stage.copy_from(meta.file_id, entry.meta.revision).await?;
        self.stage_truncate(&mut *stage, length).await?;
        self.stage_sync(&mut *stage).await?;
        self.stage_publish(&mut *stage).await?;
        self.put(path, meta).await?;
        stage.commit();
        Ok(revision)
    }

    pub async fn append(&self, path: &str, content: &[u8]) -> Result<(), Error> {
        let _execution = self.owner.execution.lock().await;
        file(path)?;
        self.require_full(path)?;
        let entry = self.entry(path).await?;
        if entry.meta.kind == FileKind::Directory {
            return Err(Error::IsDirectory);
        }
        let mut meta = entry.meta.clone();
        meta.revision = Uuid::now_v7();
        let mut stage = self.stage(meta.file_id, meta.revision).await?;
        stage.copy_from(meta.file_id, entry.meta.revision).await?;
        self.stage_write_end(&mut *stage, content).await?;
        self.stage_sync(&mut *stage).await?;
        self.stage_publish(&mut *stage).await?;
        self.put(path, meta).await?;
        stage.commit();
        Ok(())
    }

    pub async fn create_dir(&self, path: &str) -> Result<Entry<PeerId>, Error> {
        let _execution = self.owner.execution.lock().await;
        file(path)?;
        self.require_full(path)?;
        self.ensure_parent_directories(path).await?;
        match self.get(path).await? {
            Some(meta) if meta.kind == FileKind::Directory => Ok(Entry {
                path: path.to_owned(),
                meta,
            }),
            Some(_) => Err(Error::TypeConflict),
            None => {
                let meta = FileMeta::directory(Uuid::now_v7(), self.node_id.clone());
                self.put(path, meta.clone()).await?;
                Ok(Entry {
                    path: path.to_owned(),
                    meta,
                })
            }
        }
    }

    pub async fn list(&self, path: &str) -> Result<Vec<Entry<PeerId>>, Error> {
        directory(path)?;
        if !path.is_empty() {
            let entry = self.entry(path).await?;
            if entry.meta.kind != FileKind::Directory {
                return Err(Error::NotDirectory);
            }
        }
        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{path}/")
        };
        let mut entries = self
            .all_metadata()
            .await?
            .into_iter()
            .filter(|(entry_path, _)| {
                entry_path
                    .strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.contains('/'))
            })
            .map(|(path, meta)| Entry { path, meta })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(entries)
    }

    pub async fn scan(
        &self,
        path: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<ScanPage<PeerId>, Error> {
        scan_page(self.list(path).await?, cursor, limit)
    }

    pub async fn delete(&self, path: &str) -> Result<(), Error> {
        let _execution = self.owner.execution.lock().await;
        self.require_full(path)?;
        let _ = self.entry(path).await?;
        self.metadata().delete(path).await.map_err(metadata_error)?;
        let _ = self.changes.send(());
        Ok(())
    }

    #[allow(
        clippy::significant_drop_tightening,
        reason = "commit consumes the metadata transaction"
    )]
    pub async fn rename(&self, from: &str, to: &str) -> Result<(), Error> {
        let _execution = self.owner.execution.lock().await;
        file(from)?;
        file(to)?;
        self.require_full(from)?;
        self.require_full(to)?;
        let entry = self.entry(from).await?;
        if self.get(to).await?.is_some() {
            return Err(Error::AlreadyExists);
        }
        if entry.meta.kind == FileKind::Directory && to.starts_with(&format!("{from}/")) {
            return Err(Error::InvalidPath);
        }
        let mut moves = vec![(from.to_owned(), to.to_owned(), entry.meta)];
        if moves[0].2.kind == FileKind::Directory {
            let prefix = format!("{from}/");
            for (path, meta) in self.all_metadata().await? {
                if path.starts_with(&prefix) {
                    moves.push((path.clone(), format!("{to}{}", &path[from.len()..]), meta));
                }
            }
        }
        for (source, destination, _) in &moves {
            self.require_full(source)?;
            self.require_full(destination)?;
            if destination != to && self.get(destination).await?.is_some() {
                return Err(Error::AlreadyExists);
            }
        }
        self.ensure_parent_directories(to).await?;
        let mut prepared = Vec::new();
        let mut destinations = Vec::new();
        for (_, destination, meta) in &moves {
            let source_revision = meta.revision;
            let mut meta = meta.clone();
            meta.revision = Uuid::now_v7();
            if meta.kind == FileKind::File
                && self
                    .content_store()
                    .has(meta.file_id, source_revision)
                    .await?
            {
                let mut stage = self.stage(meta.file_id, meta.revision).await?;
                stage.copy_from(meta.file_id, source_revision).await?;
                self.stage_sync(&mut *stage).await?;
                self.stage_publish(&mut *stage).await?;
                prepared.push(stage);
            }
            destinations.push((destination, metadata_value(&meta)?));
        }
        let mut transaction = self
            .metadata()
            .transaction()
            .await
            .map_err(metadata_error)?;
        for (destination, value) in destinations {
            transaction
                .set(destination, value, None)
                .await
                .map_err(metadata_error)?;
        }
        for (source, _, _) in moves {
            transaction.delete(&source).await.map_err(metadata_error)?;
        }
        transaction.commit().await.map_err(metadata_error)?;
        for mut stage in prepared {
            stage.commit();
        }
        let _ = self.changes.send(());
        Ok(())
    }

    fn require_full(&self, path: &str) -> Result<(), Error> {
        (self.residency(path)? == Residency::Full)
            .then_some(())
            .ok_or(Error::PassthroughWrite)
    }

    async fn verify_content(&self, path: &str) -> Result<(), Error> {
        for (entry_path, meta) in self.all_metadata().await? {
            if within(path, &entry_path)
                && meta.kind == FileKind::File
                && !self
                    .content_store()
                    .has(meta.file_id, meta.revision)
                    .await?
            {
                return Err(Error::ContentUnavailable);
            }
        }
        Ok(())
    }

    async fn evict_unreferenced(&self, path: &str) -> Result<(), Error> {
        let entries = self.all_metadata().await?;
        for (entry_path, meta) in &entries {
            if within(path, entry_path) && meta.kind == FileKind::File {
                let file_id = meta.file_id;
                let retained = entries.iter().any(|(other_path, other)| {
                    other_path != entry_path
                        && other.kind == FileKind::File
                        && other.file_id == file_id
                        && matches!(self.residency(other_path), Ok(Residency::Full))
                });
                if !retained {
                    self.content_store().remove(file_id).await?;
                }
            }
        }
        Ok(())
    }

    async fn ensure_parent_directories(&self, path: &str) -> Result<(), Error> {
        let mut parent = String::new();
        let components = path.split('/').collect::<Vec<_>>();
        for component in &components[..components.len() - 1] {
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push_str(component);
            match self.get(&parent).await? {
                Some(meta) if meta.kind == FileKind::Directory => {}
                Some(_) => return Err(Error::NotDirectory),
                None => {
                    self.put(
                        &parent,
                        FileMeta::directory(Uuid::now_v7(), self.node_id.clone()),
                    )
                    .await?;
                }
            }
        }
        Ok(())
    }

    pub async fn revision(&self, path: &str) -> Result<Uuid, Error> {
        self.get(path)
            .await?
            .map(|meta| meta.revision)
            .ok_or(Error::NotFound)
    }

    pub(crate) async fn content(&self, file_id: Uuid, revision: Uuid) -> Result<Vec<u8>, Error> {
        if !self.has_revision(file_id, revision).await? {
            return Err(Error::ContentUnavailable);
        }
        self.content_store().read(file_id, revision).await
    }

    pub(crate) async fn store_content<S>(
        &self,
        file_id: Uuid,
        revision: Uuid,
        stream: S,
        control: &OperationControl<PeerId>,
        peer: PeerId,
    ) -> Result<(), Error>
    where
        S: futures_core::Stream<Item = Result<Bytes, Error>>,
    {
        if !self
            .owner
            .wait(control.wait(self.has_revision(file_id, revision)))
            .await???
        {
            return Err(Error::StaleRevision);
        }
        let mut stage = self.stage(file_id, revision).await?;
        futures_util::pin_mut!(stream);
        while let Some(chunk) = self.owner.wait(control.wait(stream.next())).await?? {
            self.stage_write(&mut *stage, &chunk?).await?;
        }
        self.stage_sync(&mut *stage).await?;
        let _execution = self
            .owner
            .wait(control.wait(self.owner.execution.lock()))
            .await??;
        if !self.has_revision(file_id, revision).await? {
            return Err(Error::StaleRevision);
        }
        self.owner
            .wait(control.admit(peer, RemoteOperation::PublishContent { file_id, revision }))
            .await??;
        self.owner.check()?;
        if !self.content_store().has(file_id, revision).await? {
            self.stage_publish(&mut *stage).await?;
            stage.commit();
        }
        Ok(())
    }

    async fn stage(&self, file_id: Uuid, revision: Uuid) -> Result<Box<dyn StagedObject>, Error> {
        self.content_store().stage(file_id, revision).await
    }

    async fn stage_write(&self, stage: &mut dyn StagedObject, bytes: &[u8]) -> Result<(), Error> {
        #[cfg(test)]
        self.content_checkpoint(ContentStep::Write)?;
        stage.write(bytes).await
    }

    async fn stage_write_at(
        &self,
        stage: &mut dyn StagedObject,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), Error> {
        #[cfg(test)]
        self.content_checkpoint(ContentStep::Write)?;
        stage.write_at(offset, bytes).await
    }

    async fn stage_write_end(
        &self,
        stage: &mut dyn StagedObject,
        bytes: &[u8],
    ) -> Result<(), Error> {
        #[cfg(test)]
        self.content_checkpoint(ContentStep::Write)?;
        stage.write_end(bytes).await
    }

    async fn stage_truncate(&self, stage: &mut dyn StagedObject, length: u64) -> Result<(), Error> {
        #[cfg(test)]
        self.content_checkpoint(ContentStep::Write)?;
        stage.truncate(length).await
    }

    async fn stage_sync(&self, stage: &mut dyn StagedObject) -> Result<(), Error> {
        #[cfg(test)]
        self.content_checkpoint(ContentStep::Sync)?;
        stage.sync().await
    }

    async fn stage_publish(&self, stage: &mut dyn StagedObject) -> Result<(), Error> {
        #[cfg(test)]
        self.content_checkpoint(ContentStep::Publish)?;
        stage.publish().await
    }

    #[cfg(test)]
    fn content_checkpoint(&self, step: ContentStep) -> Result<(), Error> {
        let action = {
            let mut action = self.content_action.lock().expect("content action lock");
            if action.as_ref().is_some_and(|(at, _)| *at == step) {
                action.take()
            } else {
                None
            }
        };
        if let Some((_, action)) = action {
            action();
        }
        let injected = {
            let mut fault = self.content_fault.lock().expect("content fault lock");
            if *fault == Some(step) {
                fault.take();
                true
            } else {
                false
            }
        };
        if injected {
            Err(io::Error::other(format!("injected {step:?} failure")).into())
        } else {
            Ok(())
        }
    }

    async fn has_revision(&self, file_id: Uuid, revision: Uuid) -> Result<bool, Error> {
        Ok(self
            .all_metadata()
            .await?
            .into_iter()
            .any(|(_, meta)| meta.revision == revision && meta.file_id == file_id))
    }

    pub(crate) async fn get(&self, path: &str) -> Result<Option<FileMeta<PeerId>>, Error> {
        self.metadata()
            .get(path)
            .await
            .map_err(metadata_error)?
            .map(|value| serde_json::from_value(decode_metadata(value)?).map_err(metadata_error))
            .transpose()
    }

    async fn all_metadata(&self) -> Result<Vec<(String, FileMeta<PeerId>)>, Error> {
        self.metadata()
            .scan_all()
            .await
            .map_err(metadata_error)?
            .into_iter()
            .map(|(path, value)| {
                serde_json::from_value(decode_metadata(value)?)
                    .map(|meta| (path, meta))
                    .map_err(metadata_error)
            })
            .collect()
    }

    async fn put(&self, path: &str, meta: FileMeta<PeerId>) -> Result<(), Error> {
        #[cfg(test)]
        self.content_checkpoint(ContentStep::Metadata)?;
        self.metadata()
            .set(path, metadata_value(&meta)?, None)
            .await
            .map_err(metadata_error)?;
        let _ = self.changes.send(());
        Ok(())
    }
}

fn within(scope: &str, path: &str) -> bool {
    scope.is_empty() || path == scope || path.starts_with(&format!("{scope}/"))
}

fn metadata_value<PeerId>(meta: &FileMeta<PeerId>) -> Result<Value, Error>
where
    PeerId: Serialize,
{
    serde_json::to_vec(meta)
        .map(Value::Blob)
        .map_err(metadata_error)
}

fn decode_metadata(value: Value) -> Result<serde_json::Value, Error> {
    match value {
        Value::Blob(bytes) => serde_json::from_slice(&bytes).map_err(metadata_error),
        Value::Json(value) => serde_json::Value::try_from(value).map_err(metadata_error),
        _ => Err(metadata_error("metadata value is not JSON")),
    }
}

fn metadata_error(error: impl Display) -> Error {
    Error::Metadata(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{FileMeta, Uuid, Value, decode_metadata, metadata_value};

    #[test]
    fn metadata_codec_writes_blob_json_and_reads_legacy_json() {
        let meta = FileMeta::file(Uuid::now_v7(), 1_u8);
        let expected = serde_json::to_value(&meta).expect("metadata JSON");
        let encoded = metadata_value(&meta).expect("encode metadata");
        assert!(matches!(encoded, Value::Blob(_)));
        assert_eq!(decode_metadata(encoded).expect("decode blob"), expected);

        let legacy = Value::Json(ofdb_kv::JsonValue::from(expected.clone()));
        assert_eq!(
            decode_metadata(legacy).expect("decode legacy JSON"),
            expected
        );
    }
}
