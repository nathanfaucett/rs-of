use std::{io, path::Path, sync::Mutex as StateMutex};

use tokio::sync::{Mutex, MutexGuard};
use tokio_util::sync::CancellationToken;

use crate::{native_host::NativeHost, onboarding::Failure, setup_reset};

#[derive(Default)]
pub(crate) struct Lifecycle {
    pub(crate) operation: Mutex<()>,
    state: StateMutex<State>,
}

#[derive(Default)]
struct State {
    cancellation: CancellationToken,
    closed: bool,
}

impl Lifecycle {
    pub(crate) async fn recover_references(
        &self,
        data: &Path,
        host: &Mutex<Option<NativeHost>>,
        failure: &Mutex<Option<Failure>>,
    ) -> io::Result<()> {
        let _operation = self.stop(false).await;
        *failure.lock().await = Some(Failure::Retry);
        if let Some(mut host) = host.lock().await.take() {
            host.close().await?;
        }
        setup_reset::recover(data)
    }

    pub(crate) async fn start(&self) -> Option<(MutexGuard<'_, ()>, CancellationToken)> {
        let operation = self.operation.lock().await;
        let mut state = self.state.lock().expect("lifecycle state lock");
        if state.closed {
            return None;
        }
        state.cancellation = CancellationToken::new();
        Some((operation, state.cancellation.clone()))
    }

    pub(crate) fn publish_host(
        &self,
        cancellation: &CancellationToken,
        host: NativeHost,
        slot: &mut Option<NativeHost>,
    ) -> Option<NativeHost> {
        let state = self.state.lock().expect("lifecycle state lock");
        if state.closed || cancellation.is_cancelled() || slot.is_some() {
            return Some(host);
        }
        *slot = Some(host);
        None
    }

    pub(crate) async fn stop(&self, close: bool) -> MutexGuard<'_, ()> {
        {
            let mut state = self.state.lock().expect("lifecycle state lock");
            state.closed |= close;
            state.cancellation.cancel();
        }
        self.operation.lock().await
    }
}
