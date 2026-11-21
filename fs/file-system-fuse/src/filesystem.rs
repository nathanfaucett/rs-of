use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fmt,
    future::ready,
    io::ErrorKind,
    mem,
    num::NonZeroU32,
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};

use file_system::{
    Entry, Error, FileHandle, FileKind, FileSystem, LocalFileHandle, MAX_FILE_READ_LENGTH,
    OpenRequest,
};
use fuse3::{
    Errno, FileType, Result, SetAttr,
    path::{
        PathFilesystem, Request,
        reply::{
            DirectoryEntry, DirectoryEntryPlus, FileAttr, ReplyAttr, ReplyCreated, ReplyData,
            ReplyDirectory, ReplyDirectoryPlus, ReplyEntry, ReplyInit, ReplyOpen, ReplyWrite,
        },
    },
};
use futures_util::{Stream, stream};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::Mutex;

use crate::path::{child_path, relative_path};

const MAX_OPEN_HANDLES: usize = 4096;
const FOPEN_DIRECT_IO: u32 = 1;
const NO_CACHE: Duration = Duration::ZERO;

type SharedFileHandle<PeerId> = Arc<Mutex<Option<LocalFileHandle<PeerId>>>>;

type WeakFileHandle<PeerId> = Weak<Mutex<Option<LocalFileHandle<PeerId>>>>;

struct MountedFile<PeerId>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + 'static,
{
    path: String,
    handle: SharedFileHandle<PeerId>,
}

pub struct AdapterState<PeerId>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    file_system: FileSystem<PeerId>,
    uid: u32,
    gid: u32,
    mount_time: SystemTime,
    mutations: Mutex<()>,
    next_handle: AtomicU64,
    files: Mutex<BTreeMap<u64, MountedFile<PeerId>>>,
    open_files: Mutex<BTreeMap<String, WeakFileHandle<PeerId>>>,
    directories: Mutex<BTreeMap<u64, Vec<DirectoryEntryPlus>>>,
}

impl<PeerId> AdapterState<PeerId>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    pub fn new(
        file_system: FileSystem<PeerId>,
        uid: u32,
        gid: u32,
        mount_time: SystemTime,
    ) -> Self {
        Self {
            file_system,
            uid,
            gid,
            mount_time,
            mutations: Mutex::new(()),
            next_handle: AtomicU64::new(1),
            files: Mutex::new(BTreeMap::new()),
            open_files: Mutex::new(BTreeMap::new()),
            directories: Mutex::new(BTreeMap::new()),
        }
    }

    fn handle_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed)
    }

    async fn insert_file(
        &self,
        path: String,
        handle: LocalFileHandle<PeerId>,
    ) -> Result<(u64, SharedFileHandle<PeerId>)> {
        let mut open_files = self.open_files.lock().await;
        open_files.retain(|_path, handle| handle.strong_count() != 0);
        let shared = if let Some(shared) = open_files.get(&path).and_then(Weak::upgrade) {
            let guard = shared.lock().await;
            let current = guard.as_ref().ok_or_else(|| Errno::from(libc::EBADF))?;
            if current.revision() != handle.revision() {
                return Err(Errno::from(libc::ESTALE));
            }
            drop(guard);
            shared
        } else {
            let shared = Arc::new(Mutex::new(Some(handle)));
            open_files.insert(path.clone(), Arc::downgrade(&shared));
            shared
        };
        let mut files = self.files.lock().await;
        if files.len() >= MAX_OPEN_HANDLES {
            if Arc::strong_count(&shared) == 1 {
                open_files.remove(&path);
            }
            return Err(Errno::from(libc::EMFILE));
        }
        let id = self.handle_id();
        files.insert(
            id,
            MountedFile {
                path,
                handle: Arc::clone(&shared),
            },
        );
        drop(files);
        drop(open_files);
        Ok((id, shared))
    }

    async fn insert_directory(&self, entries: Vec<DirectoryEntryPlus>) -> Result<u64> {
        let mut directories = self.directories.lock().await;
        if directories.len() >= MAX_OPEN_HANDLES {
            return Err(Errno::from(libc::EMFILE));
        }
        let id = self.handle_id();
        directories.insert(id, entries);
        drop(directories);
        Ok(id)
    }

    async fn file(&self, id: u64) -> Result<SharedFileHandle<PeerId>> {
        self.files
            .lock()
            .await
            .get(&id)
            .map(|file| Arc::clone(&file.handle))
            .ok_or_else(|| Errno::from(libc::EBADF))
    }

    async fn shared_file(&self, path: &str) -> Option<SharedFileHandle<PeerId>> {
        self.open_files
            .lock()
            .await
            .get(path)
            .and_then(Weak::upgrade)
    }

    async fn close_file(&self, id: u64) -> Result<()> {
        let mut open_files = self.open_files.lock().await;
        let mut files = self.files.lock().await;
        let file = files.remove(&id).ok_or_else(|| Errno::from(libc::EBADF))?;
        if files
            .values()
            .any(|candidate| Arc::ptr_eq(&candidate.handle, &file.handle))
        {
            return Ok(());
        }
        open_files.remove(&file.path);
        drop(files);
        let handle = file
            .handle
            .lock()
            .await
            .take()
            .expect("open file handle is present");
        drop(open_files);
        handle.close().await.map_err(errno)
    }

    pub async fn close_all(&self) {
        let _mutation = self.mutations.lock().await;
        let files = mem::take(&mut *self.files.lock().await);
        self.open_files.lock().await.clear();
        self.directories.lock().await.clear();
        for (_, file) in files {
            let handle = file.handle.lock().await.take();
            if let Some(handle) = handle {
                let _ = handle.close().await;
            }
        }
    }

    async fn entry(&self, path: &str) -> Result<Option<Entry<PeerId>>> {
        if path.is_empty() {
            return Ok(None);
        }
        self.file_system.entry(path).await.map(Some).map_err(errno)
    }

    async fn require_directory(&self, path: &str) -> Result<()> {
        if let Some(entry) = self.entry(path).await?
            && entry.meta.kind != FileKind::Directory
        {
            return Err(Errno::from(libc::ENOTDIR));
        }
        Ok(())
    }

    async fn attributes(&self, path: &str) -> Result<FileAttr> {
        let entry = self.entry(path).await?;
        let (kind, mode, size) = match entry {
            None => (FileKind::Directory, 0o755, 0),
            Some(entry) => {
                let size = match entry.meta.kind {
                    FileKind::Directory => 0,
                    FileKind::File => self
                        .file_system
                        .local_content_len(path)
                        .await
                        .map_err(errno)?,
                };
                (entry.meta.kind, entry.meta.mode, size)
            }
        };
        Ok(FileAttr {
            size,
            blocks: size.div_ceil(512),
            atime: self.mount_time,
            mtime: self.mount_time,
            ctime: self.mount_time,
            kind: file_type(kind),
            perm: u16::try_from(mode & 0o7777).expect("permission bits fit in u16"),
            nlink: if kind == FileKind::Directory { 2 } else { 1 },
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: 4096,
        })
    }
}

pub struct FuseFileSystem<PeerId>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    state: Arc<AdapterState<PeerId>>,
}

impl<PeerId> FuseFileSystem<PeerId>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    pub const fn new(state: Arc<AdapterState<PeerId>>) -> Self {
        Self { state }
    }
}

impl<PeerId> PathFilesystem for FuseFileSystem<PeerId>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    fn init(&self, _request: Request) -> impl Future<Output = Result<ReplyInit>> + Send {
        ready(Ok(ReplyInit::new(
            NonZeroU32::new(1024 * 1024).expect("nonzero FUSE write size"),
        )))
    }

    async fn destroy(&self, _request: Request) {
        self.state.close_all().await;
    }

    async fn lookup(&self, _request: Request, parent: &OsStr, name: &OsStr) -> Result<ReplyEntry> {
        let parent_path = relative_path(parent)?;
        let path = child_path(parent, name)?;
        self.state.require_directory(&parent_path).await?;
        self.state
            .entry(&path)
            .await?
            .ok_or_else(Errno::new_not_exist)?;
        Ok(ReplyEntry {
            ttl: NO_CACHE,
            attr: self.state.attributes(&path).await?,
        })
    }

    async fn getattr(
        &self,
        _request: Request,
        path: Option<&OsStr>,
        _fh: Option<u64>,
        _flags: u32,
    ) -> Result<ReplyAttr> {
        let path = path.ok_or_else(Errno::new_not_exist)?;
        let path = relative_path(path)?;
        Ok(ReplyAttr {
            ttl: NO_CACHE,
            attr: self.state.attributes(&path).await?,
        })
    }

    async fn setattr(
        &self,
        _request: Request,
        path: Option<&OsStr>,
        fh: Option<u64>,
        set_attr: SetAttr,
    ) -> Result<ReplyAttr> {
        if set_attr.mode.is_some()
            || set_attr.uid.is_some()
            || set_attr.gid.is_some()
            || set_attr.atime.is_some()
            || set_attr.mtime.is_some()
            || set_attr.ctime.is_some()
        {
            return Err(Errno::from(libc::ENOSYS));
        }
        let path = path.ok_or_else(Errno::new_not_exist)?;
        let path = relative_path(path)?;
        let Some(size) = set_attr.size else {
            return Ok(ReplyAttr {
                ttl: NO_CACHE,
                attr: self.state.attributes(&path).await?,
            });
        };
        let _mutation = self.state.mutations.lock().await;
        if let Some(fh) = fh {
            let shared = self.state.file(fh).await?;
            let mut guard = shared.lock().await;
            let handle = guard.as_mut().ok_or_else(|| Errno::from(libc::EBADF))?;
            handle.truncate(size).await.map_err(errno)?;
            drop(guard);
        } else if let Some(shared) = self.state.shared_file(&path).await {
            let mut guard = shared.lock().await;
            let handle = guard.as_mut().ok_or_else(|| Errno::from(libc::EBADF))?;
            handle.truncate(size).await.map_err(errno)?;
            drop(guard);
        } else {
            let mut handle = self
                .state
                .file_system
                .open_handle(OpenRequest {
                    path: path.clone(),
                    revision: None,
                })
                .await
                .map_err(errno)?;
            handle.truncate(size).await.map_err(errno)?;
            handle.close().await.map_err(errno)?;
        }
        Ok(ReplyAttr {
            ttl: NO_CACHE,
            attr: self.state.attributes(&path).await?,
        })
    }

    async fn open(&self, _request: Request, path: &OsStr, flags: u32) -> Result<ReplyOpen> {
        let path = relative_path(path)?;
        let truncate = flags & libc::O_TRUNC.unsigned_abs() != 0;
        let _mutation = if truncate {
            Some(self.state.mutations.lock().await)
        } else {
            None
        };
        let entry = self
            .state
            .entry(&path)
            .await?
            .ok_or_else(Errno::new_not_exist)?;
        if entry.meta.kind == FileKind::Directory {
            return Err(Errno::from(libc::EISDIR));
        }
        let handle = self
            .state
            .file_system
            .open_handle(OpenRequest {
                path: path.clone(),
                revision: None,
            })
            .await
            .map_err(errno)?;
        let (fh, shared) = self.state.insert_file(path, handle).await?;
        if truncate {
            let mut guard = shared.lock().await;
            let result = guard
                .as_mut()
                .expect("inserted file handle is present")
                .truncate(0)
                .await;
            drop(guard);
            if let Err(error) = result {
                let _ = self.state.close_file(fh).await;
                return Err(errno(error));
            }
        }
        Ok(ReplyOpen {
            fh,
            flags: FOPEN_DIRECT_IO,
        })
    }

    async fn create(
        &self,
        _request: Request,
        parent: &OsStr,
        name: &OsStr,
        _mode: u32,
        flags: u32,
    ) -> Result<ReplyCreated> {
        let truncate = flags & libc::O_TRUNC.unsigned_abs() != 0;
        let parent_path = relative_path(parent)?;
        let path = child_path(parent, name)?;
        let _mutation = self.state.mutations.lock().await;
        self.state.require_directory(&parent_path).await?;
        let entry = match self.state.file_system.create_file(&path).await {
            Ok(entry) => entry,
            Err(Error::AlreadyExists) if flags & libc::O_EXCL.unsigned_abs() != 0 => {
                return Err(Errno::from(libc::EEXIST));
            }
            Err(Error::AlreadyExists) => {
                let entry = self
                    .state
                    .entry(&path)
                    .await?
                    .ok_or_else(Errno::new_not_exist)?;
                if entry.meta.kind == FileKind::Directory {
                    return Err(Errno::from(libc::EISDIR));
                }
                entry
            }
            Err(error) => return Err(errno(error)),
        };
        let handle = self
            .state
            .file_system
            .open_handle(OpenRequest {
                path: path.clone(),
                revision: Some(entry.meta.revision),
            })
            .await
            .map_err(errno)?;
        let (fh, shared) = self.state.insert_file(path.clone(), handle).await?;
        if truncate {
            let mut guard = shared.lock().await;
            let result = guard
                .as_mut()
                .expect("inserted file handle is present")
                .truncate(0)
                .await;
            drop(guard);
            if let Err(error) = result {
                let _ = self.state.close_file(fh).await;
                return Err(errno(error));
            }
        }
        let attr = match self.state.attributes(&path).await {
            Ok(attr) => attr,
            Err(error) => {
                let _ = self.state.close_file(fh).await;
                return Err(error);
            }
        };
        Ok(ReplyCreated {
            ttl: NO_CACHE,
            attr,
            generation: 0,
            fh,
            flags: FOPEN_DIRECT_IO,
        })
    }

    async fn mkdir(
        &self,
        _request: Request,
        parent: &OsStr,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
    ) -> Result<ReplyEntry> {
        let parent_path = relative_path(parent)?;
        let path = child_path(parent, name)?;
        let _mutation = self.state.mutations.lock().await;
        self.state.require_directory(&parent_path).await?;
        match self.state.file_system.entry(&path).await {
            Ok(_entry) => return Err(Errno::from(libc::EEXIST)),
            Err(Error::NotFound) => {}
            Err(error) => return Err(errno(error)),
        }
        self.state
            .file_system
            .create_dir(&path)
            .await
            .map_err(errno)?;
        Ok(ReplyEntry {
            ttl: NO_CACHE,
            attr: self.state.attributes(&path).await?,
        })
    }

    async fn unlink(&self, _request: Request, parent: &OsStr, name: &OsStr) -> Result<()> {
        let parent_path = relative_path(parent)?;
        let path = child_path(parent, name)?;
        let _mutation = self.state.mutations.lock().await;
        self.state.require_directory(&parent_path).await?;
        let entry = self
            .state
            .entry(&path)
            .await?
            .ok_or_else(Errno::new_not_exist)?;
        if entry.meta.kind == FileKind::Directory {
            return Err(Errno::from(libc::EISDIR));
        }
        self.state.file_system.delete(&path).await.map_err(errno)
    }

    async fn rmdir(&self, _request: Request, parent: &OsStr, name: &OsStr) -> Result<()> {
        let parent_path = relative_path(parent)?;
        let path = child_path(parent, name)?;
        let _mutation = self.state.mutations.lock().await;
        self.state.require_directory(&parent_path).await?;
        let entry = self
            .state
            .entry(&path)
            .await?
            .ok_or_else(Errno::new_not_exist)?;
        if entry.meta.kind != FileKind::Directory {
            return Err(Errno::from(libc::ENOTDIR));
        }
        if !self
            .state
            .file_system
            .list(&path)
            .await
            .map_err(errno)?
            .is_empty()
        {
            return Err(Errno::from(libc::ENOTEMPTY));
        }
        self.state.file_system.delete(&path).await.map_err(errno)
    }

    async fn rename(
        &self,
        _request: Request,
        origin_parent: &OsStr,
        origin_name: &OsStr,
        parent: &OsStr,
        name: &OsStr,
    ) -> Result<()> {
        let origin_parent_path = relative_path(origin_parent)?;
        let from = child_path(origin_parent, origin_name)?;
        let parent_path = relative_path(parent)?;
        let to = child_path(parent, name)?;
        let _mutation = self.state.mutations.lock().await;
        self.state.require_directory(&origin_parent_path).await?;
        self.state.require_directory(&parent_path).await?;
        self.state
            .file_system
            .rename(&from, &to)
            .await
            .map_err(errno)
    }

    async fn rename2(
        &self,
        req: Request,
        origin_parent: &OsStr,
        origin_name: &OsStr,
        parent: &OsStr,
        name: &OsStr,
        flags: u32,
    ) -> Result<()> {
        if flags & !libc::RENAME_NOREPLACE != 0 {
            return Err(Errno::from(libc::ENOSYS));
        }
        self.rename(req, origin_parent, origin_name, parent, name)
            .await
    }

    async fn read(
        &self,
        _request: Request,
        _path: Option<&OsStr>,
        fh: u64,
        offset: u64,
        size: u32,
    ) -> Result<ReplyData> {
        if size > MAX_FILE_READ_LENGTH {
            return Err(Errno::from(libc::EINVAL));
        }
        let shared = self.state.file(fh).await?;
        let mut guard = shared.lock().await;
        let handle = guard.as_mut().ok_or_else(|| Errno::from(libc::EBADF))?;
        let data = handle.read(offset, size).await.map_err(errno)?;
        drop(guard);
        Ok(ReplyData { data })
    }

    async fn write(
        &self,
        _request: Request,
        path: Option<&OsStr>,
        fh: u64,
        offset: u64,
        data: &[u8],
        _write_flags: u32,
        flags: u32,
    ) -> Result<ReplyWrite> {
        let path = path.ok_or_else(Errno::new_not_exist)?;
        let path = relative_path(path)?;
        let _mutation = self.state.mutations.lock().await;
        let shared = self.state.file(fh).await?;
        let mut guard = shared.lock().await;
        let handle = guard.as_mut().ok_or_else(|| Errno::from(libc::EBADF))?;
        let offset = if flags & libc::O_APPEND.unsigned_abs() != 0 {
            self.state
                .file_system
                .local_content_len(&path)
                .await
                .map_err(errno)?
        } else {
            offset
        };
        let written = handle
            .write(offset, data.to_vec().into())
            .await
            .map_err(errno)?;
        drop(guard);
        Ok(ReplyWrite { written })
    }

    async fn flush(
        &self,
        _request: Request,
        _path: Option<&OsStr>,
        fh: u64,
        _lock_owner: u64,
    ) -> Result<()> {
        let shared = self.state.file(fh).await?;
        let guard = shared.lock().await;
        guard.as_ref().ok_or_else(|| Errno::from(libc::EBADF))?;
        drop(guard);
        Ok(())
    }

    async fn fsync(
        &self,
        _request: Request,
        _path: Option<&OsStr>,
        fh: u64,
        _datasync: bool,
    ) -> Result<()> {
        let shared = self.state.file(fh).await?;
        let guard = shared.lock().await;
        guard.as_ref().ok_or_else(|| Errno::from(libc::EBADF))?;
        drop(guard);
        Ok(())
    }

    async fn release(
        &self,
        _request: Request,
        _path: Option<&OsStr>,
        fh: u64,
        _flags: u32,
        _lock_owner: u64,
        _flush: bool,
    ) -> Result<()> {
        let _mutation = self.state.mutations.lock().await;
        self.state.close_file(fh).await
    }

    async fn opendir(&self, _request: Request, path: &OsStr, flags: u32) -> Result<ReplyOpen> {
        reject_mutation_flags(flags)?;
        let path = relative_path(path)?;
        self.state.require_directory(&path).await?;
        let entries = self.state.file_system.list(&path).await.map_err(errno)?;
        let mut snapshot = Vec::with_capacity(entries.len() + 2);
        snapshot.push(DirectoryEntryPlus {
            kind: FileType::Directory,
            name: OsString::from("."),
            offset: 1,
            attr: self.state.attributes(&path).await?,
            entry_ttl: NO_CACHE,
            attr_ttl: NO_CACHE,
        });
        let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
        snapshot.push(DirectoryEntryPlus {
            kind: FileType::Directory,
            name: OsString::from(".."),
            offset: 2,
            attr: self.state.attributes(parent).await?,
            entry_ttl: NO_CACHE,
            attr_ttl: NO_CACHE,
        });
        for entry in entries {
            let offset =
                i64::try_from(snapshot.len() + 1).map_err(|_error| Errno::from(libc::EOVERFLOW))?;
            snapshot.push(DirectoryEntryPlus {
                kind: file_type(entry.meta.kind),
                name: OsString::from(
                    entry
                        .path
                        .rsplit('/')
                        .next()
                        .expect("entry has a final component"),
                ),
                offset,
                attr: self.state.attributes(&entry.path).await?,
                entry_ttl: NO_CACHE,
                attr_ttl: NO_CACHE,
            });
        }
        let fh = self.state.insert_directory(snapshot).await?;
        Ok(ReplyOpen { fh, flags: 0 })
    }

    async fn readdir<'a>(
        &'a self,
        _request: Request,
        _path: &'a OsStr,
        fh: u64,
        offset: i64,
    ) -> Result<ReplyDirectory<impl Stream<Item = Result<DirectoryEntry>> + Send + 'a>> {
        if offset < 0 {
            return Err(Errno::from(libc::EINVAL));
        }
        let start = usize::try_from(offset).map_err(|_error| Errno::from(libc::EINVAL))?;
        let entries = self
            .state
            .directories
            .lock()
            .await
            .get(&fh)
            .cloned()
            .ok_or_else(|| Errno::from(libc::EBADF))?;
        Ok(ReplyDirectory {
            entries: stream::iter(entries.into_iter().skip(start).map(|entry| {
                Ok(DirectoryEntry {
                    kind: entry.kind,
                    name: entry.name,
                    offset: entry.offset,
                })
            })),
        })
    }

    async fn readdirplus<'a>(
        &'a self,
        _request: Request,
        _parent: &'a OsStr,
        fh: u64,
        offset: u64,
        _lock_owner: u64,
    ) -> Result<ReplyDirectoryPlus<impl Stream<Item = Result<DirectoryEntryPlus>> + Send + 'a>>
    {
        let start = usize::try_from(offset).map_err(|_error| Errno::from(libc::EINVAL))?;
        let entries = self
            .state
            .directories
            .lock()
            .await
            .get(&fh)
            .cloned()
            .ok_or_else(|| Errno::from(libc::EBADF))?;
        Ok(ReplyDirectoryPlus {
            entries: stream::iter(entries.into_iter().skip(start).map(Ok)),
        })
    }

    async fn releasedir(
        &self,
        _request: Request,
        _path: &OsStr,
        fh: u64,
        _flags: u32,
    ) -> Result<()> {
        self.state
            .directories
            .lock()
            .await
            .remove(&fh)
            .map(|_entries| ())
            .ok_or_else(|| Errno::from(libc::EBADF))
    }
}

fn reject_mutation_flags(flags: u32) -> Result<()> {
    let mutation_flags =
        (libc::O_APPEND | libc::O_CREAT | libc::O_EXCL | libc::O_TRUNC).unsigned_abs();
    if flags & libc::O_ACCMODE.unsigned_abs() != libc::O_RDONLY.unsigned_abs()
        || flags & mutation_flags != 0
    {
        return Err(Errno::from(libc::EROFS));
    }
    Ok(())
}

const fn file_type(kind: FileKind) -> FileType {
    match kind {
        FileKind::Directory => FileType::Directory,
        FileKind::File => FileType::RegularFile,
    }
}

fn errno(error: Error) -> Errno {
    let code = match error {
        Error::InvalidPath
        | Error::InvalidResourceId
        | Error::InvalidChunkSize
        | Error::InvalidReadLength
        | Error::InvalidScanLimit
        | Error::InvalidScanCursor => libc::EINVAL,
        Error::NotFound => libc::ENOENT,
        Error::NotDirectory | Error::TypeConflict => libc::ENOTDIR,
        Error::IsDirectory => libc::EISDIR,
        Error::AlreadyExists => libc::EEXIST,
        Error::StaleRevision => libc::ESTALE,
        Error::PassthroughWrite => libc::EROFS,
        Error::PermissionDenied => libc::EACCES,
        Error::Cancelled => libc::ECANCELED,
        Error::Io(error)
            if error.raw_os_error().is_none() && error.kind() == ErrorKind::InvalidInput =>
        {
            libc::EINVAL
        }
        Error::Io(error) => return Errno::from(error),
        Error::ContentUnavailable
        | Error::Offline
        | Error::NoPeers
        | Error::NoReachableProvider
        | Error::UnsupportedSyncProtocolVersion(_)
        | Error::SyncFrameTooLarge
        | Error::InvalidSyncFrame(_)
        | Error::Metadata(_) => libc::EIO,
    };
    Errno::from(code)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, time::UNIX_EPOCH};

    use fuse3::Timestamp;
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn rename2_rejects_unsupported_flags_without_changing_entries() {
        let root = env::temp_dir().join(format!("offs-fuse-rename2-{}", Uuid::now_v7()));
        let file_system = FileSystem::open(&root, 1_u8).await.expect("open");
        file_system
            .set_residency("", file_system::Residency::Full)
            .await
            .expect("full residency");
        let source = file_system
            .write("source", b"source")
            .await
            .expect("source");
        let occupied = file_system
            .write("occupied", b"occupied")
            .await
            .expect("occupied");
        let adapter = FuseFileSystem::new(Arc::new(AdapterState::new(
            file_system.clone(),
            0,
            0,
            UNIX_EPOCH,
        )));

        for flags in [
            libc::RENAME_EXCHANGE,
            libc::RENAME_WHITEOUT,
            libc::RENAME_EXCHANGE | libc::RENAME_NOREPLACE,
            1 << 31,
        ] {
            let result = adapter
                .rename2(
                    Request::default(),
                    OsStr::new("/"),
                    OsStr::new("source"),
                    OsStr::new("/"),
                    OsStr::new("occupied"),
                    flags,
                )
                .await;
            assert!(matches!(result, Err(error) if error == Errno::from(libc::ENOSYS)));
            assert_eq!(
                file_system
                    .entry("source")
                    .await
                    .expect("source preserved")
                    .meta,
                source.meta
            );
            assert_eq!(
                file_system
                    .entry("occupied")
                    .await
                    .expect("destination preserved")
                    .meta,
                occupied.meta
            );
        }
        assert_eq!(
            file_system.read("source").await.expect("source bytes"),
            b"source"
        );
        assert_eq!(
            file_system
                .read("occupied")
                .await
                .expect("destination bytes"),
            b"occupied"
        );
        drop(adapter);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[tokio::test]
    async fn setattr_rejects_every_unsupported_component_before_size_mutation() {
        let root = env::temp_dir().join(format!("offs-fuse-setattr-{}", Uuid::now_v7()));
        let file_system = FileSystem::open(&root, 1_u8).await.expect("open");
        file_system
            .set_residency("", file_system::Residency::Full)
            .await
            .expect("full residency");
        let original = file_system.write("file", b"original").await.expect("write");
        let state = Arc::new(AdapterState::new(file_system.clone(), 0, 0, UNIX_EPOCH));
        let adapter = FuseFileSystem::new(state);
        let timestamp = Some(Timestamp::new(1, 0));
        let requests = [
            SetAttr {
                size: Some(0),
                mode: Some(0o600),
                ..SetAttr::default()
            },
            SetAttr {
                size: Some(0),
                uid: Some(1),
                ..SetAttr::default()
            },
            SetAttr {
                size: Some(0),
                gid: Some(1),
                ..SetAttr::default()
            },
            SetAttr {
                size: Some(0),
                atime: timestamp,
                ..SetAttr::default()
            },
            SetAttr {
                size: Some(0),
                mtime: timestamp,
                ..SetAttr::default()
            },
            SetAttr {
                size: Some(0),
                ctime: timestamp,
                ..SetAttr::default()
            },
        ];

        for request in requests {
            let result = adapter
                .setattr(Request::default(), Some(OsStr::new("/file")), None, request)
                .await;
            assert!(matches!(result, Err(error) if error == Errno::from(libc::ENOSYS)));
            assert_eq!(
                file_system.read("file").await.expect("old bytes"),
                b"original"
            );
            assert_eq!(
                file_system.revision("file").await.expect("old revision"),
                original.meta.revision
            );
        }

        let reply = adapter
            .setattr(
                Request::default(),
                Some(OsStr::new("/file")),
                None,
                SetAttr {
                    size: Some(4),
                    lock_owner: Some(7),
                    ..SetAttr::default()
                },
            )
            .await
            .expect("size with lock context");
        assert_eq!(reply.attr.size, 4);
        assert_eq!(file_system.read("file").await.expect("new bytes"), b"orig");
        drop(adapter);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
