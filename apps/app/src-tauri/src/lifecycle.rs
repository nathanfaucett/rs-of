use std::sync::Mutex as StateMutex;

use tokio::sync::{Mutex, MutexGuard};
use tokio_util::sync::CancellationToken;

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
        host: crate::native_host::NativeHost,
        slot: &mut Option<crate::native_host::NativeHost>,
    ) -> Option<crate::native_host::NativeHost> {
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
