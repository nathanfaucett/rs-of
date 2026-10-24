use std::{collections::HashMap, io, sync::Arc};

use idp_service::repo::PrivateKeyKeyringRepo;
use keyring_core::{Entry, api::CredentialStoreApi};

struct TestStore(Arc<keyring_core::mock::Store>);
impl CredentialStoreApi for TestStore {
    fn vendor(&self) -> String {
        self.0.vendor()
    }
    fn id(&self) -> String {
        self.0.id()
    }
    fn build(
        &self,
        service: &str,
        user: &str,
        _: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<Entry> {
        self.0.build(service, user, None)
    }
    fn search(&self, spec: &HashMap<&str, &str>) -> keyring_core::Result<Vec<Entry>> {
        self.0.search(spec)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn main() -> io::Result<()> {
    PrivateKeyKeyringRepo::new_with_store(
        "system-test",
        Arc::new(TestStore(
            keyring_core::mock::Store::new().expect("isolated mock provider"),
        )),
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
