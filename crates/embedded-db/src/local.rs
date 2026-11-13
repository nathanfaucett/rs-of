use std::{io, path::Path, sync::Arc};

use db::Client;
use of_model::db::Database;

pub fn open_private_database(path: &Path) -> io::Result<db::Database> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(file) => file.sync_all()?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local database must be a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "local database must deny group and other access",
            ));
        }
    }
    db::Database::open(path).map_err(io::Error::other)
}

/// Opens a fresh local owner database, or uses the supplied query client unchanged.
pub fn local_client(
    data_dir: &Path,
    owner: &str,
    database: Option<Arc<dyn Database>>,
) -> io::Result<Client> {
    if let Some(database) = database {
        return Ok(database.client());
    }
    if !matches!(owner, "idp" | "device" | "storage" | "management") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown local owner",
        ));
    }
    std::fs::create_dir_all(data_dir)?;
    let database = open_private_database(&data_dir.join(format!("{owner}.redb")))?;
    let client = database.client();
    // Storage's public constructor is synchronous, including inside a Tokio runtime.
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(of_model::replica::up(&database))
            .map_err(io::Error::other)
    })
    .join()
    .map_err(|_| io::Error::other("local schema worker panicked"))??;
    Ok(client)
}
