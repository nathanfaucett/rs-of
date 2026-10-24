use std::{io, sync::Arc};

use idp_service::repo::PrivateKeyKeyringRepo;

#[path = "system_test/provider.rs"]
mod provider;

fn main() -> io::Result<()> {
    PrivateKeyKeyringRepo::new_with_store(
        "system-test",
        Arc::new(provider::Store::open(
            std::env::var_os("OF_SYSTEM_PROVIDER")
                .expect("private durable provider path")
                .into(),
        )?),
    );
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_stack_size(64 * 1024 * 1024)
                .enable_all()
                .build()?
                .block_on(async {
                    match std::env::var("OF_SYSTEM_SERVICE").as_deref() {
                        Ok("unified") => unified_server::run().await,
                        Ok("idp") => idp_server::run().await,
                        Ok("device") => device_server::run().await,
                        Ok("management") => management_server::run().await,
                        Ok("storage") => storage_server::run().await,
                        _ => Err(io::Error::other("expected an explicit system-test service")),
                    }
                })
        })?
        .join()
        .expect("service thread")
}
