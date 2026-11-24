use std::{fmt, io, thread};

use tokio::runtime::Builder;

/// Runs a persistence future on a dedicated current-thread runtime.
///
/// # Errors
///
/// Returns an error if the runtime cannot be built, the worker panics, or the future fails.
pub fn run<T: Send>(future: impl Future<Output = io::Result<T>> + Send) -> io::Result<T> {
    thread::scope(|scope| {
        scope
            .spawn(|| {
                Builder::new_current_thread()
                    .enable_all()
                    .build()?
                    .block_on(future)
            })
            .join()
            .map_err(|_error| io::Error::other("local persistence worker panicked"))?
    })
}

pub fn error(error: impl fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}
