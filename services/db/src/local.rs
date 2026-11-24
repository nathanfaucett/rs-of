use std::{
    fs::{self, OpenOptions},
    io,
    path::Path,
    sync::Arc,
};

use btree_redb::SharedDatabase;
use ofdb_sql::Client;
use ofdb_sql::Database as SqlDatabase;

use crate::adapters::EmbeddedDatabase;
use crate::ofdb::up;

/// Creates a private storage file with secure permissions (0o600 on Unix).
///
/// # Errors
///
/// Returns an error if file creation or validation fails, permissions are too broad, or the
/// storage cannot be opened.
pub fn open_private_storage(path: &Path) -> io::Result<SharedDatabase> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600)
    };
    match options.open(path) {
        Ok(file) => file.sync_all()?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(path)?;
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
    SharedDatabase::create(path)
}

/// Opens a local database file, runs migrations, and returns the SQL client.
///
/// # Errors
///
/// Returns an error if storage setup, database opening, or schema migration fails.
pub async fn local_client(
    data_dir: &Path,
    injected: Option<Arc<SqlDatabase>>,
) -> io::Result<Client> {
    let db = local_database(data_dir, injected).await?;
    Ok(db.client())
}

/// Opens the local unified database.
///
/// The database file is `<data_dir>/unified.redb`. Migrations run before the
/// `EmbeddedDatabase` adapter is returned. If `injected` is provided, the adapter wraps it with
/// dummy storage.
///
/// # Errors
///
/// Returns an error if storage setup, database opening, or schema migration fails.
pub async fn local_database(
    data_dir: &Path,
    injected: Option<Arc<SqlDatabase>>,
) -> Result<Arc<EmbeddedDatabase>, io::Error> {
    if let Some(database) = injected {
        // Shared database provided by unified server — no storage access.
        // Clone the SqlDatabase from the Arc (SqlDatabase is cheap to clone).
        let database = (*database).clone();
        let adapter = EmbeddedDatabase {
            database,
            storage: SharedDatabase::in_memory()
                .map_err(|error| io::Error::other(error.to_string()))?,
        };
        return Ok(Arc::new(adapter));
    }
    fs::create_dir_all(data_dir)?;
    let path = data_dir.join("unified.redb");
    let storage = open_private_storage(&path)?;
    let database = SqlDatabase::open_shared(&storage, "control")
        .map_err(|error| io::Error::other(error.to_string()))?;
    let adapter = EmbeddedDatabase { database, storage };
    up(&adapter.client())
        .await
        .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(Arc::new(adapter))
}
