use std::{
    fs::{self, File, OpenOptions},
    future::Future,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    pin::Pin,
};

// Construct large owner futures outside the caller's poll frame as well as storing them on the heap.
#[inline(never)]
pub fn provisioning_stage<'a, T: 'a, F>(
    make: impl FnOnce() -> F,
) -> Pin<Box<dyn Future<Output = io::Result<T>> + Send + 'a>>
where
    F: Future<Output = io::Result<T>> + Send + 'a,
{
    Box::pin(make())
}

pub fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(invalid("empty paths and parent components are not allowed"));
    }
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(path
        .components()
        .filter(|part| !matches!(part, Component::CurDir))
        .collect())
}

pub fn reject_symlinks(path: &Path) -> io::Result<()> {
    for ancestor in absolute(path)?.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(invalid("symlink ancestry is not allowed"));
            }
            Ok(_) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub fn require_absent(path: &Path) -> io::Result<()> {
    reject_symlinks(path)?;
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(invalid(
            "fresh setup requires absent roots; existing state requires exact continuation or recovery",
        )),
    }
}

pub fn require_retained_directory(path: &Path) -> io::Result<fs::Metadata> {
    reject_symlinks(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(invalid("managed root must be a regular directory"));
    }
    Ok(metadata)
}

pub fn require_directory(path: &Path) -> io::Result<()> {
    private_permissions(&require_retained_directory(path)?)
}

pub fn require_file(path: &Path, private: bool) -> io::Result<File> {
    reject_symlinks(path)?;
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(invalid("input or retained state must be a regular file"));
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(invalid("input or retained state must be a regular file"));
    }
    if private {
        private_permissions(&metadata)?;
    }
    Ok(file)
}

fn private_permissions(metadata: &fs::Metadata) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 == 0 {
            return Ok(());
        }
        Err(invalid(
            "operator files/directories must deny group and other access",
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "local provisioning requires Unix owner-only permissions; no Windows ACL qualification",
        ))
    }
}

pub fn require_unix_bootstrap() -> io::Result<()> {
    #[cfg(unix)]
    {
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Node bootstrap supports Unix owner-only permissions only",
        ))
    }
}

pub fn create_private_directory(path: &Path, recursive: bool) -> io::Result<()> {
    require_unix_bootstrap()?;
    reject_symlinks(path)?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.recursive(recursive).create(path)?;
    sync_ancestry(path)
}

pub fn private_options(create_new: bool) -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(create_new);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

pub fn lock(directory: &Path, fresh: bool) -> io::Result<File> {
    require_directory(directory)?;
    let path = directory.join("operation.lock");
    if !fresh {
        require_file(&path, true)?;
    }
    let file = private_options(fresh).open(path)?;
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => io::Error::new(
            io::ErrorKind::WouldBlock,
            "another local provisioning/runtime operation owns this host",
        ),
        fs::TryLockError::Error(error) => error,
    })?;
    file.sync_all()?;
    sync_ancestry(directory)?;
    Ok(file)
}

pub fn read_private(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = require_file(path, true)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn read_password(path: &Path) -> io::Result<String> {
    let mut password = String::from_utf8(read_private(path)?)
        .map_err(|_| invalid("password input must be UTF-8"))?;
    if password.ends_with('\n') {
        password.pop();
        if password.ends_with('\r') {
            password.pop();
        }
    }
    validate_password(&password)?;
    Ok(password)
}

pub fn validate_password(password: &str) -> io::Result<()> {
    if password.trim().is_empty() || password.contains(['\r', '\n']) {
        return Err(invalid("password input must contain one nonempty line"));
    }
    Ok(())
}

pub fn sync_ancestry(directory: &Path) -> io::Result<()> {
    for ancestor in directory.ancestors() {
        File::open(ancestor)?.sync_all()?;
    }
    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
