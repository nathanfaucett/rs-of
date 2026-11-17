use std::{future::Future, io};

pub fn run<T: Send>(future: impl Future<Output = io::Result<T>> + Send) -> io::Result<T> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?
                    .block_on(future)
            })
            .join()
            .map_err(|_| io::Error::other("local persistence worker panicked"))?
    })
}

pub fn error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}
