use std::{io, sync::Arc};

use iroh_chain::Server;
use tokio::sync::watch;

use crate::{DataProtocolHandler, StorageSyncDriver};

#[derive(Clone)]
pub struct StorageNetwork {
    pub server: Server,
    pub policy_changes: ofdb_sync::ChangeSubscription,
    install: Arc<dyn Fn(DataProtocolHandler) -> io::Result<()> + Send + Sync>,
}
impl StorageNetwork {
    pub fn new(
        server: Server,
        policy_changes: ofdb_sync::ChangeSubscription,
        install: impl Fn(DataProtocolHandler) -> io::Result<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            server,
            policy_changes,
            install: Arc::new(install),
        }
    }
    pub(crate) fn install(&self, handler: DataProtocolHandler) -> io::Result<()> {
        (self.install)(handler)
    }
}

pub struct StorageNativeHost {
    pub network: watch::Receiver<Option<StorageNetwork>>,
    pub driver: watch::Sender<Option<StorageSyncDriver>>,
}
