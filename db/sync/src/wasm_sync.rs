// WASM-specific push synchronization using web-sys timers and futures::select!
// Does not require tokio runtime.

use crate::protocol::{
    Config, HelloConfig, Message, PushResult, PushSynchronizeConfig, PushSynchronizeError,
};
use btree::{ChangeEvent, StorageKey};
use core::pin::Pin;
use core::task::{Context, Poll};
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use futures::future::{pending, ready};
use futures::select;
use std::collections::HashMap;
use std::mem::take;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use web_sys::window;

// Type aliases for WASM compatibility (expected by ofdb-wasm crate)
pub type PushConfig = Config;
pub type PushMessage = Message;
// PushResult is re-exported from protocol
// PushSynchronizeConfig is re-exported from protocol
// PushSynchronizeError is re-exported from protocol
pub use crate::transport::SyncTransport as PushSyncTransport;

/// Timer future using web-sys setTimeout/clearTimeout.
struct WasmSleep {
    duration: Duration,
    handle: Option<i32>,
    done: bool,
}

impl WasmSleep {
    const fn new(duration: Duration) -> Self {
        Self {
            duration,
            handle: None,
            done: false,
        }
    }
}

impl Future for WasmSleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.done {
            return Poll::Ready(());
        }

        if self.handle.is_none() {
            let window = window().expect("no window available");
            let waker = cx.waker().clone();
            let millis = i32::try_from(self.duration.as_millis().min(i32::MAX as u128))
                .expect("duration is clamped to i32::MAX");

            let closure = Closure::once_into_js(move || {
                waker.wake();
            });

            let handle = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(),
                    millis,
                )
                .expect("setTimeout failed");

            self.handle = Some(handle);
        }

        Poll::Pending
    }
}

impl Drop for WasmSleep {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take()
            && let Some(window) = window()
        {
            window.clear_timeout_with_handle(handle);
        }
    }
}

/// Sleep for a duration using web-sys timers.
fn wasm_sleep(duration: Duration) -> impl Future<Output = ()> {
    WasmSleep::new(duration)
}

/// Timeout wrapper using web-sys timers and `futures::select`.
/// Does not require the future to be Unpin.
async fn wasm_timeout<F, T>(duration: Duration, future: F) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    let timer = wasm_sleep(duration);
    futures::pin_mut!(future, timer);
    select! {
        res = future.fuse() => Ok(res),
        () = timer.fuse() => Err(()),



    }
}

/// Run push-based synchronization between two peers (WASM version).
///
/// This version uses web-sys timers and `futures::select`!, allowing
/// non-Send futures to be used without a tokio runtime.
///
/// # Type Parameters
/// - `T`: Transport implementing `PushSyncTransport`
/// - `F`: Callback to apply inbound `ChangeEvent` to local storage
///
/// # Arguments
/// - `transport`: Bidirectional transport
/// - `outbound`: Stream of local `ChangeEvent` to push to peer
/// - `apply_inbound`: Function to apply received `ChangeEvent` to local storage
/// - `config`: Synchronization configuration
///
/// # Returns
/// `PushResult` with statistics, or `PushSynchronizeError` on failure.
#[allow(
    clippy::too_many_lines,
    reason = "one select loop coordinates protocol timeout and termination state"
)]
pub fn push_synchronize<T, F, S, Fut>(
    mut transport: T,
    mut outbound: S,
    mut apply_inbound: F,
    config: PushSynchronizeConfig,
) -> impl Future<Output = Result<PushResult, PushSynchronizeError<T::Error>>>
where
    T: PushSyncTransport,
    S: Stream<Item = ChangeEvent> + Unpin,
    F: FnMut(ChangeEvent) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let handshake_timeout = Duration::from_millis(config.handshake_timeout_ms);
    let message_timeout = Duration::from_millis(config.message_timeout_ms);
    async move {
        let mut result = PushResult::default();
        let mut local_generations: HashMap<StorageKey, u64> = HashMap::new();
        let mut pending_acks: HashMap<StorageKey, u64> = HashMap::new();
        let mut outstanding = HashMap::new();
        let mut sent_done = false;
        let mut received_done = false;
        let mut outbound_exhausted = false;
        let mut inbound_closed = false;

        let hello = Message::Hello(HelloConfig {
            protocol_version: 1,
            manifest_digest: [0; 32],
            config: config.protocol.clone(),
        });
        // Perform handshake with timeout
        let handshake_result = wasm_timeout(handshake_timeout, async {
            match config.role {
                crate::SyncRole::Initiator => {
                    transport
                        .send(hello)
                        .await
                        .map_err(PushSynchronizeError::Transport)?;
                    match transport
                        .receive()
                        .await
                        .map_err(PushSynchronizeError::Transport)?
                    {
                        Message::HelloAck {
                            protocol_version: 1,
                            accepted: true,
                        } => Ok(()),
                        _ => Err(PushSynchronizeError::HandshakeFailed(
                            "expected accepted version-one HelloAck".into(),
                        )),
                    }
                }
                crate::SyncRole::Responder => {
                    let accepted = matches!(
                        transport
                            .receive()
                            .await
                            .map_err(PushSynchronizeError::Transport)?,
                        Message::Hello(HelloConfig {
                            protocol_version: 1,
                            ..
                        })
                    );
                    transport
                        .send(Message::HelloAck {
                            protocol_version: 1,
                            accepted,
                        })
                        .await
                        .map_err(PushSynchronizeError::Transport)?;
                    if accepted {
                        Ok(())
                    } else {
                        Err(PushSynchronizeError::HandshakeFailed(
                            "unsupported Hello".into(),
                        ))
                    }
                }
            }
        })
        .await;
        let _ = handshake_result.map_err(|()| PushSynchronizeError::SessionTimeout)?;

        // Main bidirectional loop using futures::select!
        loop {
            select! {
                // Outbound: send local `ChangeEvent`s
                event = async {
                    if outbound_exhausted {
                        pending().await
                    } else {
                        outbound.next().await
                    }
                }.fuse() => {
                    if let Some(event) = event {
                        // Track local generation
                        let generation = event.generation;
                        local_generations.insert(event.key.clone(), generation);
                        outstanding.insert(event.key.clone(), generation);

                        let msg = Message::ChangeEvent(event);
                        transport.send(msg).await
                            .map_err(PushSynchronizeError::Transport)?;
                        result.sent += 1;
                    } else {
                        outbound_exhausted = true;
                        if config.send_done_on_exhaust && !sent_done {
                            let done = Message::Done { final_generations: local_generations.clone() };
                            transport.send(done).await
                                .map_err(PushSynchronizeError::Transport)?;
                            sent_done = true;
                        }
                    }
                }

                // Inbound: receive and process messages with message timeout
                msg = async {
                    if inbound_closed {
                        pending().await
                    } else {
                        wasm_timeout(message_timeout, transport.receive()).await
                    }
                }.fuse() => {
                    match msg {
                        Ok(Ok(message)) => {
                            match message {
                                Message::ChangeEvent(event) => {
                                    let generation = event.generation;
                                    let key = event.key.clone();

                                    // Apply to local storage
                                    apply_inbound(event).await
                                        .map_err(PushSynchronizeError::Apply)?;
                                    result.received += 1;

                                    // Queue acknowledgment
                                    pending_acks.insert(key, generation);

                                    // Send ACK if immediate
                                    if config.protocol.immediate_ack {
                                        let ack = Message::Ack {
                                            acknowledgments: take(&mut pending_acks),
                                        };
                                        transport.send(ack).await
                                            .map_err(PushSynchronizeError::Transport)?;
                                    }
                                }
                                Message::SnapshotRequest { .. } => {
                                    // TODO: Implement snapshot serving
                                    // For now, send Close to indicate we can't serve
                                    let close = Message::Close;
                                    transport.send(close).await
                                        .map_err(PushSynchronizeError::Transport)?;
                                }
                                Message::Snapshot { key, blob, generation } => {
                                    // Apply snapshot
                                    let event = ChangeEvent { key, blob, generation };
                                    apply_inbound(event).await
                                        .map_err(PushSynchronizeError::Apply)?;
                                }
                                Message::Ack { acknowledgments } => {
                                    // Peer acknowledged our events
                                    for (key, _generation) in acknowledgments {
                                        pending_acks.remove(&key);
                                    }
                                }
                                Message::Done { .. } => {
                                    received_done = true;
                                }
                                Message::Close => {
                                    inbound_closed = true;
                                }
                                Message::HelloAck { .. } | Message::Hello { .. } => {
                                    // Ignore duplicate handshake messages
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            return Err(PushSynchronizeError::Transport(e));
                        }
                        Err(()) => {
                            return Err(PushSynchronizeError::SessionTimeout);
                        }
                    }
                }

                // Periodic ACK flush if not immediate ack
                () = async {
                    if config.protocol.immediate_ack || pending_acks.is_empty() {
                        pending::<()>().await;
                    } else {
                        wasm_sleep(Duration::from_millis(100)).await;
                    }
                }.fuse() => {
                    let ack = Message::Ack {
                        acknowledgments: take(&mut pending_acks),
                    };
                    transport.send(ack).await
                        .map_err(PushSynchronizeError::Transport)?;
                }

                // Termination check
                () = async {
                    if outbound_exhausted && outstanding.is_empty() && (received_done || inbound_closed) {
                        ready(()).await;
                    } else {
                        pending::<()>().await;
                    }
                }.fuse() => {
                    break;
                }
            }

            if sent_done && received_done {
                break;
            }
        }

        // Send final DONE if not already sent
        if !sent_done {
            let done = Message::Done {
                final_generations: local_generations.clone(),
            };
            transport
                .send(done)
                .await
                .map_err(PushSynchronizeError::Transport)?;
        }

        // Send final CLOSE
        let close = Message::Close;
        let _ = transport.send(close).await;

        Ok(result)
    }
}

/// Configuration for `push_synchronize` (re-export with expected name).
pub type SynchronizeConfig = PushSynchronizeConfig;

/// Type alias for error (re-export with expected name).
pub type SynchronizeError<E> = PushSynchronizeError<E>;

/// Run push-based synchronization (re-export with expected name).
pub fn synchronize<T, F, S, Fut>(
    transport: T,
    outbound: S,
    apply_inbound: F,
    config: SynchronizeConfig,
) -> impl Future<Output = Result<PushResult, PushSynchronizeError<T::Error>>>
where
    T: PushSyncTransport,
    S: Stream<Item = ChangeEvent> + Unpin,
    F: FnMut(ChangeEvent) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    push_synchronize(transport, outbound, apply_inbound, config)
}
