use std::{io, path::PathBuf, sync::Arc};

use api::IndependentCreateControl;
use db::Database;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub(crate) fn socket_path(control: &IndependentCreateControl) -> io::Result<PathBuf> {
    Ok(control.directory().canonicalize()?.join("identity.sock"))
}

pub(crate) async fn serve(
    control: IndependentCreateControl,
    mut owners: watch::Receiver<Option<Arc<Database>>>,
    shutdown: CancellationToken,
) -> io::Result<()> {
    let owner = loop {
        if let Some(owner) = owners.borrow_and_update().clone() {
            break owner;
        }
        tokio::select! {
            biased;
            () = shutdown.cancelled() => return Ok(()),
            result = owners.changed() => result.map_err(io::Error::other)?,
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;

        let path = socket_path(&control)?;
        if let Ok(metadata) = std::fs::symlink_metadata(&path) {
            if !metadata.file_type().is_socket() {
                return Err(io::Error::other(
                    "unexpected identity socket path; preserve evidence",
                ));
            }
            std::fs::remove_file(&path)?;
        }
        let result = owner
            .serve_unix(&path, shutdown.cancelled_owned())
            .await
            .map_err(io::Error::other);
        if path.try_exists()? {
            std::fs::remove_file(path)?;
        }
        result
    }
    #[cfg(not(unix))]
    {
        let _ = (owner, control);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "local independent identity requires Unix sockets; configure remote identity SQL on this platform",
        ))
    }
}
