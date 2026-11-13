use std::{io, path::Path, sync::Arc};

use db::Client;
use of_model::db::Database;

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
    let database =
        db::Database::open(data_dir.join(format!("{owner}.redb"))).map_err(io::Error::other)?;
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
