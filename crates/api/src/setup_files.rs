use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

pub(crate) fn require_unix_bootstrap() -> io::Result<()> {
    if cfg!(unix) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setup requires Unix owner-only retention",
        ))
    }
}

pub(crate) fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(io::Error::other(
            "empty paths and parent components are not allowed",
        ));
    }
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let path: PathBuf = path
        .components()
        .filter(|part| !matches!(part, Component::CurDir))
        .collect();
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(io::Error::other("symlink ancestry is not allowed"));
            }
            Ok(_) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
    }
    Ok(path)
}

pub(crate) fn present(path: &Path) -> io::Result<bool> {
    let path = absolute(path)?;
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(crate) fn require_absent(path: &Path) -> io::Result<()> {
    if present(path)? {
        Err(io::Error::other(
            "fresh setup requires absent roots; exact recovery required",
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn create_private_directory(path: &Path, recursive: bool) -> io::Result<()> {
    require_unix_bootstrap()?;
    let path = absolute(path)?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.recursive(recursive).create(&path)?;
    sync_ancestry(&path)
}

pub(crate) fn sync_ancestry(path: &Path) -> io::Result<()> {
    for ancestor in absolute(path)?.ancestors() {
        File::open(ancestor)?.sync_all()?;
    }
    Ok(())
}

pub(crate) fn private_options(create_new: bool) -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(create_new);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(
            rustix::fs::OFlags::NOFOLLOW.bits() as i32 | rustix::fs::OFlags::NONBLOCK.bits() as i32,
        );
    }
    options
}

pub(crate) fn require_private(path: &Path, directory: bool) -> io::Result<()> {
    require_unix_bootstrap()?;
    let path = absolute(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err(io::Error::other("retained setup material has invalid type"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0 || metadata.uid() != rustix::process::geteuid().as_raw() {
            return Err(io::Error::other(
                "retained setup material must be owner-only and euid-owned",
            ));
        }
    }
    Ok(())
}

pub(crate) fn read_private(path: &Path) -> io::Result<Vec<u8>> {
    require_private(path, false)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(
            rustix::fs::OFlags::NOFOLLOW.bits() as i32 | rustix::fs::OFlags::NONBLOCK.bits() as i32,
        );
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.mode() & 0o077 != 0
            || metadata.uid() != rustix::process::geteuid().as_raw()
        {
            return Err(io::Error::other("invalid protected retained file"));
        }
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}
