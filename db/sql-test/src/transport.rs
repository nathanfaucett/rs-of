use core::{fmt, future::ready};

use std::error::Error;

use futures::{StreamExt, channel::mpsc};

use sync::{Message, SyncTransport};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InMemoryTransportError {
    Closed,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportDirection {
    LeftToRight,
    RightToLeft,
}

impl fmt::Display for InMemoryTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Closed => "channel closed",
            Self::Interrupted => "transport interrupted",
        })
    }
}

impl Error for InMemoryTransportError {}

#[derive(Debug)]
pub struct InMemoryTransport {
    receiver: mpsc::UnboundedReceiver<Message>,
    sender: Option<mpsc::UnboundedSender<Message>>,
    fail_at: Option<usize>,
    sent: usize,
}

impl SyncTransport for InMemoryTransport {
    type Error = InMemoryTransportError;

    #[allow(
        clippy::map_err_ignore,
        reason = "a closed channel is the only send failure"
    )]
    fn send(&mut self, msg: Message) -> impl Future<Output = Result<(), Self::Error>> + Send {
        if self.fail_at == Some(self.sent) {
            self.sender = None;
            return ready(Err(InMemoryTransportError::Interrupted));
        }
        self.sent += 1;
        ready(
            self.sender
                .as_ref()
                .map_or(Err(InMemoryTransportError::Closed), |sender| {
                    sender
                        .unbounded_send(msg)
                        .map_err(|_| InMemoryTransportError::Closed)
                }),
        )
    }

    async fn receive(&mut self) -> Result<Message, Self::Error> {
        self.receiver
            .next()
            .await
            .ok_or(InMemoryTransportError::Closed)
    }
}

#[must_use]
pub fn in_memory_transport_pair() -> (InMemoryTransport, InMemoryTransport) {
    in_memory_transport_pair_failing_at(None)
}

#[must_use]
pub fn in_memory_transport_pair_failing_at(
    fail_at: Option<usize>,
) -> (InMemoryTransport, InMemoryTransport) {
    in_memory_transport_pair_failing(None, fail_at)
}

#[must_use]
pub fn in_memory_transport_pair_failing(
    direction: Option<TransportDirection>,
    fail_at: Option<usize>,
) -> (InMemoryTransport, InMemoryTransport) {
    let (left_sender, right_receiver) = mpsc::unbounded();
    let (right_sender, left_receiver) = mpsc::unbounded();
    let left_fails = direction != Some(TransportDirection::RightToLeft);
    let right_fails = direction == Some(TransportDirection::RightToLeft);
    (
        InMemoryTransport {
            receiver: left_receiver,
            sender: Some(left_sender),
            fail_at: left_fails.then_some(fail_at).flatten(),
            sent: 0,
        },
        InMemoryTransport {
            receiver: right_receiver,
            sender: Some(right_sender),
            fail_at: right_fails.then_some(fail_at).flatten(),
            sent: 0,
        },
    )
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::{InMemoryTransportError, SyncTransport, in_memory_transport_pair_failing_at};

    #[test]
    fn failed_send_closes_the_peer() {
        block_on(async {
            let (mut left, mut right) = in_memory_transport_pair_failing_at(Some(0));

            assert_eq!(
                left.send(sync::Message::Close).await,
                Err(InMemoryTransportError::Interrupted)
            );
            assert_eq!(right.receive().await, Err(InMemoryTransportError::Closed));
        });
    }
}
