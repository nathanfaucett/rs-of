use std::{
    future::Future,
    sync::{Arc, Mutex},
};

use iroh::EndpointId;
use iroh_chain::Server;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(crate) struct ReplicationAdmission {
    lifetime: CancellationToken,
    state: Arc<Mutex<State>>,
}

struct State {
    generation: u64,
    ready: bool,
    cancellation: CancellationToken,
}

impl ReplicationAdmission {
    pub(crate) fn new(lifetime: CancellationToken) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                generation: 0,
                ready: true,
                cancellation: lifetime.child_token(),
            })),
            lifetime,
        }
    }

    pub(crate) fn fence(&self) {
        let mut state = self
            .state
            .lock()
            .expect("replication admission is unpoisoned");
        state.cancellation.cancel();
        state.cancellation = self.lifetime.child_token();
        state.generation = state.generation.wrapping_add(1);
        state.ready = false;
    }

    pub(crate) fn generation(&self) -> u64 {
        self.state
            .lock()
            .expect("replication admission is unpoisoned")
            .generation
    }

    pub(crate) fn open(&self, generation: u64) -> bool {
        let mut state = self
            .state
            .lock()
            .expect("replication admission is unpoisoned");
        if state.generation != generation || self.lifetime.is_cancelled() {
            return false;
        }
        state.ready = true;
        true
    }

    pub(crate) fn allows(&self) -> bool {
        let state = self
            .state
            .lock()
            .expect("replication admission is unpoisoned");
        state.ready && !state.cancellation.is_cancelled()
    }

    pub(crate) async fn run<F: Future>(
        &self,
        server: &Server,
        peer: EndpointId,
        work: F,
    ) -> Option<F::Output> {
        let mut peers = server.peers().subscribe_changes();
        let cancellation = {
            let state = self
                .state
                .lock()
                .expect("replication admission is unpoisoned");
            if !state.ready || !server.peers().contains(peer) {
                return None;
            }
            state.cancellation.clone()
        };
        tokio::pin!(work);
        loop {
            tokio::select! {
                biased;
                () = cancellation.cancelled() => return None,
                result = peers.changed() => {
                    if result.is_err() || !server.peers().contains(peer) { return None; }
                }
                result = &mut work => return Some(result),
            }
        }
    }
}
