use std::{
    error::Error as StdError,
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use file_system::{FileSystem, Residency};
use fuse3::{MountOptions, path::Session, raw::MountHandle};
use nix::unistd::{getgid, getuid};
use serde::{Serialize, de::DeserializeOwned};

use crate::filesystem::{AdapterState, FuseFileSystem};

/// A mount validation or session error.
#[derive(Debug)]
pub enum MountError {
    /// A root or nested entry does not have Full Residency.
    NotFullResidency {
        /// The File System-relative path that failed validation.
        path: String,
    },
    /// A File System metadata or local-content operation failed.
    FileSystem(file_system::Error),
    /// Mountpoint validation or FUSE session setup failed.
    Io(io::Error),
}

impl fmt::Display for MountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFullResidency { path } => {
                write!(f, "mount requires Full Residency at {path:?}")
            }
            Self::FileSystem(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl StdError for MountError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::NotFullResidency { .. } => None,
            Self::FileSystem(error) => Some(error),
            Self::Io(error) => Some(error),
        }
    }
}

impl From<file_system::Error> for MountError {
    fn from(error: file_system::Error) -> Self {
        Self::FileSystem(error)
    }
}

impl From<io::Error> for MountError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Owns an active FUSE session and its adapter-owned handles.
pub struct MountGuard<PeerId>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    handle: Option<MountHandle>,
    state: Arc<AdapterState<PeerId>>,
    mountpoint: PathBuf,
}

impl<PeerId> fmt::Debug for MountGuard<PeerId>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MountGuard")
            .field("mountpoint", &self.mountpoint)
            .finish_non_exhaustive()
    }
}

impl<PeerId> MountGuard<PeerId>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    /// Returns the directory owned by this mount session.
    #[must_use]
    pub fn mountpoint(&self) -> &Path {
        &self.mountpoint
    }

    /// Stops request processing, closes adapter-owned handles, and unmounts the session.
    ///
    /// Close operating-system file and directory handles first. Open-file and pending-write
    /// shutdown hardening are not supported in this increment.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system cannot unmount the session.
    pub async fn unmount(mut self) -> io::Result<()> {
        let result = match self.handle.take() {
            Some(handle) => handle.unmount().await,
            None => Ok(()),
        };
        self.state.close_all().await;
        result
    }
}

/// Validates and mounts an opened generic File System on Linux.
///
/// The mountpoint must be an existing empty directory distinct from the backing root. The caller
/// retains Catalog, Resource, authorization, synchronization, and Residency ownership. Unsupported
/// mutation callbacks return `ENOSYS`; write/mutation open flags return `EROFS`. `allow_other` is
/// disabled, attribute and entry TTLs are zero, file reads use direct I/O, and writeback caching is
/// disabled.
///
/// # Errors
///
/// Returns an error for non-Full Residency, missing local content, an invalid mountpoint, or FUSE
/// session setup failure. Validation does not change Residency or retained content.
pub async fn mount<PeerId>(
    file_system: FileSystem<PeerId>,
    mountpoint: impl AsRef<Path>,
) -> Result<MountGuard<PeerId>, MountError>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    validate_file_system(&file_system).await?;
    let mountpoint = prepare_mountpoint(mountpoint.as_ref())?;
    let uid = getuid().as_raw();
    let gid = getgid().as_raw();
    let state = Arc::new(AdapterState::new(file_system, uid, gid, SystemTime::now()));
    let mut options = MountOptions::default();
    options
        .uid(uid)
        .gid(gid)
        .fs_name("offs")
        .allow_other(false)
        .default_permissions(true)
        .write_back(false);
    let handle = Session::new(options)
        .mount_with_unprivileged(FuseFileSystem::new(Arc::clone(&state)), &mountpoint)
        .await?;
    Ok(MountGuard {
        handle: Some(handle),
        state,
        mountpoint,
    })
}

fn prepare_mountpoint(mountpoint: &Path) -> io::Result<PathBuf> {
    if !fs::metadata(mountpoint)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mountpoint must be a directory",
        ));
    }
    if fs::read_dir(mountpoint)?.next().transpose()?.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mountpoint must be empty and distinct from the backing root",
        ));
    }
    Ok(mountpoint.to_owned())
}

async fn validate_file_system<PeerId>(file_system: &FileSystem<PeerId>) -> Result<(), MountError>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    let mut pending = vec![String::new()];
    while let Some(path) = pending.pop() {
        require_full(file_system, &path)?;
        for entry in file_system.list(&path).await? {
            require_full(file_system, &entry.path)?;
            match entry.meta.kind {
                file_system::FileKind::Directory => pending.push(entry.path),
                file_system::FileKind::File => {
                    file_system.local_content_len(&entry.path).await?;
                }
            }
        }
    }
    Ok(())
}

fn require_full<PeerId>(file_system: &FileSystem<PeerId>, path: &str) -> Result<(), MountError>
where
    PeerId: Clone + fmt::Debug + Ord + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    if file_system.residency(path)? != Residency::Full {
        return Err(MountError::NotFullResidency {
            path: path.to_owned(),
        });
    }
    Ok(())
}
