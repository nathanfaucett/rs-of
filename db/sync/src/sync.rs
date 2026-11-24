use std::{collections::HashMap, mem::take, time::Duration};

use btree::{ChangeEvent, StorageKey};
use futures::{FutureExt, Stream, StreamExt, future::pending};
use thiserror::Error;
use tokio::time::timeout;

use crate::{
    SyncRole,
    protocol::{
        Config, Error, HelloConfig, Message, PushApplyResult, PushResult, PushSynchronizeConfig,
        PushSynchronizeError, SyncResult,
    },
    transport::SyncTransport,
};

/// Error during synchronization.
#[derive(Debug, Error)]
pub enum SynchronizeError<E> {
    #[error("transport error: {0}")]
    Transport(E),
    #[error("protocol error: {0}")]
    Protocol(Error<E>),
    #[error("apply error: {0}")]
    Apply(String),
    #[error("session timeout")]
    SessionTimeout,
    #[error("peer closed")]
    PeerClosed,
    #[error("handshake failed: {0}")]
    HandshakeFailed(String),
    #[error("generation gap for key {key:?}: have {have}, need {need}")]
    GenerationGap {
        key: StorageKey,
        have: u64,
        need: u64,
    },
}

impl<E> From<Error<E>> for SynchronizeError<E> {
    fn from(e: Error<E>) -> Self {
        Self::Protocol(e)
    }
}

/// Configuration for synchronize.
#[derive(Debug, Clone)]
pub struct SynchronizeConfig {
    pub role: SyncRole,
    /// Protocol configuration.
    pub protocol: Config,
    /// Maximum time to wait for handshake.
    pub handshake_timeout: Duration,
    /// Maximum time between messages before timeout.
    pub message_timeout: Duration,
    /// Whether to send DONE after exhausting outbound stream.
    pub send_done_on_exhaust: bool,
}

impl Default for SynchronizeConfig {
    fn default() -> Self {
        Self {
            role: SyncRole::Initiator,
            protocol: Config::default(),
            handshake_timeout: Duration::from_secs(10),
            message_timeout: Duration::from_secs(30),
            send_done_on_exhaust: true,
        }
    }
}

/// Run synchronization between two peers.
///
/// This function implements the sync protocol:
/// 1. Handshake (Hello/HelloAck)
/// 2. Bidirectional streaming of `ChangeEvent`
/// 3. Snapshot fallback for missing dependencies
/// 4. Acknowledgment of received events
/// 5. Graceful close (Done/Close)
///
/// # Type Parameters
/// - `T`: Transport implementing `SyncTransport`
/// - `F`: Callback to apply inbound `ChangeEvent` to local storage
///
/// # Arguments
/// - `transport`: Bidirectional transport
/// - `outbound`: Stream of local `ChangeEvent` to push to peer
/// - `apply_inbound`: Function to apply received `ChangeEvent` to local storage
/// - `config`: Synchronization configuration
///
/// # Returns
/// `SyncResult` with statistics, or `SynchronizeError` on failure.
///
/// # Errors
///
/// Returns an error for handshake, timeout, transport, protocol, or inbound apply failures.
#[allow(
    clippy::too_many_lines,
    reason = "one select loop keeps protocol state transitions and termination together"
)]
#[allow(
    clippy::map_err_ignore,
    reason = "an elapsed timeout maps to the stable session-timeout error"
)]
pub async fn synchronize<T, F, S, Fut>(
    mut transport: T,
    mut outbound: S,
    mut apply_inbound: F,
    config: SynchronizeConfig,
) -> Result<SyncResult, SynchronizeError<T::Error>>
where
    T: SyncTransport,
    S: Stream<Item = ChangeEvent> + Unpin + Send,
    F: FnMut(ChangeEvent) -> Fut + Send,
    Fut: Future<Output = Result<PushApplyResult, String>> + Send,
{
    let mut result = SyncResult::default();
    let mut local_generations: HashMap<StorageKey, u64> = HashMap::new();

    let mut pending_acks: HashMap<StorageKey, u64> = HashMap::new();
    let mut outstanding = HashMap::new();
    let mut sent_done = false;
    let mut received_done = false;

    let hello = Message::Hello(HelloConfig {
        protocol_version: 1,
        manifest_digest: [0; 32],
        config: config.protocol.clone(),
    });
    let handshake = async {
        match config.role {
            SyncRole::Initiator => {
                transport
                    .send(hello)
                    .await
                    .map_err(SynchronizeError::Transport)?;
                match transport
                    .receive()
                    .await
                    .map_err(SynchronizeError::Transport)?
                {
                    Message::HelloAck {
                        protocol_version: 1,
                        accepted: true,
                    } => Ok(()),
                    _ => Err(SynchronizeError::HandshakeFailed(
                        "expected accepted version-one HelloAck".into(),
                    )),
                }
            }
            SyncRole::Responder => {
                let accepted = matches!(
                    transport
                        .receive()
                        .await
                        .map_err(SynchronizeError::Transport)?,
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
                    .map_err(SynchronizeError::Transport)?;
                if accepted {
                    Ok(())
                } else {
                    Err(SynchronizeError::HandshakeFailed(
                        "unsupported Hello".into(),
                    ))
                }
            }
        }
    };
    timeout(config.handshake_timeout, handshake)
        .await
        .map_err(|_| SynchronizeError::SessionTimeout)??;

    let incoming = transport.receive().fuse();
    futures::pin_mut!(incoming);

    // Main bidirectional loop
    let mut outbound_exhausted = false;
    let mut inbound_closed = false;

    loop {
        tokio::select! {
            biased;
            // Outbound: send local ChangeEvents
            event = async {
                if outbound_exhausted {
                    pending().await
                } else {
                    outbound.next().await
                }
            }, if !outbound_exhausted => {
                if let Some(event) = event {
                    // Track local generation
                    let generation = event.generation;
                    local_generations.insert(event.key.clone(), generation);
                    outstanding.insert(event.key.clone(), generation);

                    let msg = Message::ChangeEvent(event);
                    transport.send(msg).await
                        .map_err(SynchronizeError::Transport)?;
                    result.events_sent += 1;
                    result.bytes_sent += 1; // approximate
                } else {
                    outbound_exhausted = true;
                    if config.send_done_on_exhaust && !sent_done {
                        let done = Message::Done {
                            final_generations: local_generations.clone(),
                        };
                        transport.send(done).await
                            .map_err(SynchronizeError::Transport)?;
                        sent_done = true;
                    }
                }
            }
            // Inbound: receive and process messages
            msg = async {
                if inbound_closed {
                    pending().await
                } else {
                    timeout(config.message_timeout, incoming.as_mut()).await
                }
            }, if !inbound_closed => {
                if msg.is_ok() {
                    incoming.set(transport.receive().fuse());
                }
                match msg {
                    Ok(Ok(message)) => {
                        result.bytes_received += 1; // approximate
                        match message {
                            Message::ChangeEvent(event) => {
                                // Track remote generation
                                let generation = event.generation;
                                let key = event.key.clone();


                                // Apply to local storage
                                let applied = apply_inbound(event).await
                                    .map_err(SynchronizeError::Apply)?;
                                result.events_received += 1;

                                if applied == PushApplyResult::Applied {
                                    pending_acks.insert(key, generation);
                                }

                                // Send ACK if immediate
                                if config.protocol.immediate_ack && !pending_acks.is_empty() {
                                    let ack = Message::Ack {
                                        acknowledgments: take(&mut pending_acks),
                                    };
                                    transport.send(ack).await
                                        .map_err(SynchronizeError::Transport)?;
                                }
                            }
                            Message::SnapshotRequest { .. } => {
                                // TODO: Implement snapshot serving
                                // For now, send Close to indicate we can't serve
                                let close = Message::Close;
                                transport.send(close).await
                                    .map_err(SynchronizeError::Transport)?;
                            }
                            Message::Snapshot { key, blob, generation } => {
                                // Apply snapshot
                                let event = ChangeEvent { key, blob, generation };
                                apply_inbound(event).await
                                    .map_err(SynchronizeError::Apply)?;
                                result.snapshots_received += 1;
                            }
                            Message::Ack { acknowledgments } => {
                                // Peer acknowledged our events
                                for (key, generation) in acknowledgments {
                                    if outstanding.get(&key).is_some_and(|pending| *pending <= generation) { outstanding.remove(&key); }
                                }
                            }
                            Message::Done { .. } => {
                                received_done = true;
                            }
                            Message::Close => {
                                if !outstanding.is_empty() {
                                    return Err(SynchronizeError::PeerClosed);
                                }
                                inbound_closed = true;
                            }
                            Message::HelloAck { .. } | Message::Hello { .. } => {
                                // Ignore duplicate handshake messages
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        return Err(SynchronizeError::Protocol(Error::Transport(e)));
                    }
                    Err(_) => {
                        return Err(SynchronizeError::SessionTimeout);
                    }
                }
            }
            // Check termination conditions
            () = async {}, if outbound_exhausted && outstanding.is_empty() && pending_acks.is_empty() && (received_done || inbound_closed) => {
                break;
            }
        }

        // Send batched ACKs if not immediate
        if !config.protocol.immediate_ack && !pending_acks.is_empty() {
            let ack = Message::Ack {
                acknowledgments: take(&mut pending_acks),
            };
            transport
                .send(ack)
                .await
                .map_err(SynchronizeError::Transport)?;
        }
    }

    // Send final DONE if not already sent
    if !sent_done {
        let done = Message::Done {
            final_generations: local_generations,
        };
        transport
            .send(done)
            .await
            .map_err(SynchronizeError::Transport)?;
    }

    // Send final CLOSE
    let close = Message::Close;
    let _ = transport.send(close).await;

    Ok(result)
}

/// Push synchronization - one-way sync from local to remote.
///
/// This is a simplified version of synchronize that only pushes local changes
/// without expecting inbound changes from the remote.
pub async fn push_synchronize<T, F, S, Fut>(
    transport: T,
    outbound: S,
    apply_inbound: F,
    config: PushSynchronizeConfig,
) -> Result<PushResult, PushSynchronizeError<T::Error>>
where
    T: SyncTransport,
    S: Stream<Item = ChangeEvent> + Unpin + Send,
    F: FnMut(ChangeEvent) -> Fut + Send,
    Fut: Future<Output = Result<PushApplyResult, String>> + Send,
{
    // Convert PushSynchronizeConfig to SynchronizeConfig
    let sync_config = SynchronizeConfig {
        role: config.role,
        protocol: config.protocol,
        handshake_timeout: Duration::from_millis(config.handshake_timeout_ms),
        message_timeout: Duration::from_millis(config.message_timeout_ms),
        send_done_on_exhaust: true,
    };

    // Use the existing synchronize function
    let result = synchronize(transport, outbound, apply_inbound, sync_config)
        .await
        .map_err(|e| match e {
            SynchronizeError::Transport(e) => PushSynchronizeError::Transport(e),
            SynchronizeError::Protocol(e) => PushSynchronizeError::Protocol(e.to_string()),
            SynchronizeError::Apply(e) => PushSynchronizeError::Apply(e),
            SynchronizeError::SessionTimeout => {
                PushSynchronizeError::Protocol("session timeout".into())
            }
            SynchronizeError::PeerClosed => PushSynchronizeError::Protocol("peer closed".into()),
            SynchronizeError::HandshakeFailed(e) => PushSynchronizeError::Protocol(e),
            SynchronizeError::GenerationGap { key, have, need } => PushSynchronizeError::Protocol(
                format!("generation gap for key {key:?}: have {have}, need {need}"),
            ),
        })?;

    // Convert SyncResult to PushResult
    Ok(PushResult {
        sent: usize::try_from(result.events_sent).unwrap_or(usize::MAX),
        received: usize::try_from(result.events_received).unwrap_or(usize::MAX),
        conflicts_resolved: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{channel::mpsc, stream};
    use std::future::ready;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use std::task::Poll;

    struct Channel(
        mpsc::UnboundedSender<Message>,
        Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Message>>>,
    );

    impl Channel {
        fn new(
            sender: mpsc::UnboundedSender<Message>,
            receiver: mpsc::UnboundedReceiver<Message>,
        ) -> Self {
            Self(sender, Arc::new(tokio::sync::Mutex::new(receiver)))
        }
    }

    impl SyncTransport for Channel {
        type Error = String;
        fn send(&mut self, msg: Message) -> impl Future<Output = Result<(), String>> {
            ready(
                self.0
                    .unbounded_send(msg)
                    .map_err(|error| error.to_string()),
            )
        }
        fn receive(&mut self) -> impl Future<Output = Result<Message, String>> + Send + 'static {
            let receiver = Arc::clone(&self.1);
            async move {
                receiver
                    .lock()
                    .await
                    .next()
                    .await
                    .ok_or_else(|| "closed".into())
            }
        }
    }
    #[tokio::test]
    async fn initiator_and_responder_exchange_changes_and_finish() {
        let (a, ar) = mpsc::unbounded();
        let (b, br) = mpsc::unbounded();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let left_seen = Arc::clone(&seen);
        let right_seen = Arc::clone(&seen);
        let event = |key: &[u8], byte| ChangeEvent {
            key: StorageKey::kv(key.to_vec()),
            blob: vec![byte],
            generation: 1,
        };
        let result = timeout(Duration::from_secs(2), async {
            tokio::join!(
                synchronize(
                    Channel::new(a, br),
                    stream::iter([event(b"left", 7)]),
                    move |change| {
                        left_seen.lock().expect("observations").push(change.blob);
                        async { Ok(PushApplyResult::Applied) }
                    },
                    SynchronizeConfig {
                        role: SyncRole::Initiator,
                        ..Default::default()
                    }
                ),
                synchronize(
                    Channel::new(b, ar),
                    stream::iter([event(b"right", 9)]),
                    move |change| {
                        right_seen.lock().expect("observations").push(change.blob);
                        async { Ok(PushApplyResult::Applied) }
                    },
                    SynchronizeConfig {
                        role: SyncRole::Responder,
                        ..Default::default()
                    }
                ),
            )
        })
        .await
        .expect("finite sessions must complete");
        assert_eq!(result.0.expect("initiator exchange").events_received, 1);
        assert_eq!(result.1.expect("responder exchange").events_received, 1);
        let mut seen = seen.lock().expect("observations").clone();
        seen.sort();
        assert_eq!(seen, vec![vec![7], vec![9]]);
    }

    struct ScriptedTransport {
        peer: mpsc::UnboundedSender<Message>,
        receiver: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Message>>>,
        acknowledgments: HashMap<StorageKey, u64>,
        receive_calls: Arc<AtomicUsize>,
        responded: bool,
    }

    impl ScriptedTransport {
        fn new() -> (Self, Arc<AtomicUsize>) {
            let (peer, receiver) = mpsc::unbounded();
            let receive_calls = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    peer,
                    receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
                    acknowledgments: HashMap::new(),
                    receive_calls: Arc::clone(&receive_calls),
                    responded: false,
                },
                receive_calls,
            )
        }
    }

    impl SyncTransport for ScriptedTransport {
        type Error = String;

        fn send(&mut self, msg: Message) -> impl Future<Output = Result<(), String>> {
            match msg {
                Message::Hello(_) => {
                    self.peer
                        .unbounded_send(Message::HelloAck {
                            protocol_version: 1,
                            accepted: true,
                        })
                        .expect("handshake response");
                }
                Message::ChangeEvent(event) => {
                    self.acknowledgments.insert(event.key, event.generation);
                }
                Message::Done { .. } if !self.responded => {
                    self.responded = true;
                    self.peer
                        .unbounded_send(Message::ChangeEvent(ChangeEvent {
                            key: StorageKey::kv(b"remote".to_vec()),
                            blob: vec![9],
                            generation: 1,
                        }))
                        .expect("remote change");
                    self.peer
                        .unbounded_send(Message::Ack {
                            acknowledgments: take(&mut self.acknowledgments),
                        })
                        .expect("acknowledgment");
                    self.peer
                        .unbounded_send(Message::Done {
                            final_generations: HashMap::new(),
                        })
                        .expect("done");
                    self.peer.unbounded_send(Message::Close).expect("close");
                }
                _ => {}
            }
            ready(Ok(()))
        }

        fn receive(&mut self) -> impl Future<Output = Result<Message, String>> + Send + 'static {
            self.receive_calls.fetch_add(1, Ordering::SeqCst);
            let receiver = Arc::clone(&self.receiver);
            async move {
                receiver
                    .lock()
                    .await
                    .next()
                    .await
                    .ok_or_else(|| "closed".into())
            }
        }
    }

    #[tokio::test]
    async fn outbound_readiness_does_not_recreate_pending_receive() {
        let (transport, receive_calls) = ScriptedTransport::new();
        let mut index = 0;
        let mut ready_event = false;
        let outbound = stream::poll_fn(move |cx| {
            if index == 4 {
                return Poll::Ready(None);
            }
            if ready_event {
                ready_event = false;
                index += 1;
                Poll::Ready(Some(ChangeEvent {
                    key: StorageKey::kv(format!("local-{index}").into_bytes()),
                    blob: vec![7],
                    generation: 1,
                }))
            } else {
                ready_event = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        });

        let result = timeout(
            Duration::from_secs(2),
            synchronize(
                transport,
                outbound,
                |_| async { Ok(PushApplyResult::Applied) },
                SynchronizeConfig::default(),
            ),
        )
        .await
        .expect("contended exchange must complete")
        .expect("contended exchange must succeed");

        assert_eq!(result.events_sent, 4);
        assert_eq!(result.events_received, 1, "pending frame must survive");
        assert_eq!(
            receive_calls.load(Ordering::SeqCst),
            6,
            "one handshake receive, one retained receive, three replacements, and one unused close replacement"
        );
    }

    #[tokio::test]
    async fn deferred_changes_prevent_sender_success() {
        let (a, ar) = mpsc::unbounded();
        let (b, br) = mpsc::unbounded();
        let event = ChangeEvent {
            key: StorageKey::kv(b"key".to_vec()),
            blob: vec![1],
            generation: 1,
        };
        let config = |role| SynchronizeConfig {
            role,
            message_timeout: Duration::from_millis(50),
            ..Default::default()
        };
        let (initiator, responder) = timeout(Duration::from_secs(2), async {
            tokio::join!(
                synchronize(
                    Channel::new(a, br),
                    stream::iter([event]),
                    |_| async { Ok(PushApplyResult::Applied) },
                    config(SyncRole::Initiator),
                ),
                synchronize(
                    Channel::new(b, ar),
                    stream::empty(),
                    |_| async { Ok(PushApplyResult::Deferred) },
                    config(SyncRole::Responder),
                ),
            )
        })
        .await
        .expect("incomplete exchange must terminate");

        assert!(matches!(initiator, Err(SynchronizeError::PeerClosed)));
        responder.expect("receiver finishes the protocol before its facade validates pending work");
    }
}
