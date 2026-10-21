use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, Weak},
};

use iroh::EndpointId;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard, watch};
use tokio_util::sync::CancellationToken;

use crate::database_protocol::DatabaseResourceDescriptor;

type Key = (EndpointId, String, String, String);

#[derive(Default)]
pub(crate) struct DatabaseSessions(Mutex<BTreeMap<Key, Weak<Session>>>);

struct Session {
    serial: Arc<AsyncMutex<()>>,
    outbound: watch::Sender<Option<CancellationToken>>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            serial: Arc::default(),
            outbound: watch::channel(None).0,
        }
    }
}

pub(crate) struct SessionPermit {
    session: Arc<Session>,
    _guard: OwnedMutexGuard<()>,
    pub(crate) cancellation: CancellationToken,
    outbound: bool,
}

impl DatabaseSessions {
    fn session(&self, peer: EndpointId, resource: &DatabaseResourceDescriptor) -> Arc<Session> {
        let mut sessions = self.0.lock().expect("database session map is unpoisoned");
        sessions.retain(|_, session| session.strong_count() > 0);
        let key = (
            peer,
            resource.owner_subject.clone(),
            resource.application_id.clone(),
            resource.database_id.clone(),
        );
        if let Some(session) = sessions.get(&key).and_then(Weak::upgrade) {
            return session;
        }
        let session = Arc::new(Session::default());
        sessions.insert(key, Arc::downgrade(&session));
        session
    }

    pub(crate) async fn outbound(
        &self,
        peer: EndpointId,
        resource: &DatabaseResourceDescriptor,
    ) -> SessionPermit {
        let session = self.session(peer, resource);
        let guard = session.serial.clone().lock_owned().await;
        let cancellation = CancellationToken::new();
        session.outbound.send_replace(Some(cancellation.clone()));
        SessionPermit {
            session,
            _guard: guard,
            cancellation,
            outbound: true,
        }
    }

    pub(crate) async fn inbound(
        &self,
        local: EndpointId,
        peer: EndpointId,
        resource: &DatabaseResourceDescriptor,
    ) -> SessionPermit {
        let session = self.session(peer, resource);
        let prefer_inbound = local.to_string() > peer.to_string();
        let mut outbound = session.outbound.subscribe();
        let acquire = session.serial.clone().lock_owned();
        tokio::pin!(acquire);
        let guard = loop {
            // Publication can follow serial-lock acquisition on another thread.
            // Keep observing it while waiting so the preferred inbound breaks inversion.
            if prefer_inbound && let Some(cancellation) = outbound.borrow_and_update().as_ref() {
                cancellation.cancel();
            }
            tokio::select! {
                biased;
                _ = outbound.changed(), if prefer_inbound => {}
                guard = &mut acquire => break guard,
            }
        };
        SessionPermit {
            session,
            _guard: guard,
            cancellation: CancellationToken::new(),
            outbound: false,
        }
    }
}

impl Drop for SessionPermit {
    fn drop(&mut self) {
        if self.outbound {
            self.session.outbound.send_replace(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use core::{
        future::{Future, poll_fn},
        task::Poll,
    };

    use tokio_util::sync::CancellationToken;

    use super::{DatabaseSessions, SessionPermit};
    use crate::database_protocol::DatabaseResourceDescriptor;

    #[tokio::test]
    async fn inbound_observes_outbound_token_published_after_it_started_waiting() {
        let lower = DatabaseSessions::default();
        let higher = DatabaseSessions::default();
        let mut ids = [
            iroh::SecretKey::generate().public(),
            iroh::SecretKey::generate().public(),
        ];
        ids.sort_by_key(|id| id.to_string());
        let resource = DatabaseResourceDescriptor {
            owner_subject: "owner".into(),
            application_id: "app".into(),
            database_id: "db".into(),
        };
        let lower_outbound = lower.outbound(ids[1], &resource).await;
        let session = higher.session(ids[0], &resource);
        // Hold the exact outbound seam: serial acquired, token not yet published.
        let higher_outbound = SessionPermit {
            _guard: session.serial.clone().lock_owned().await,
            session: session.clone(),
            cancellation: CancellationToken::new(),
            outbound: true,
        };
        let higher_inbound = higher.inbound(ids[1], ids[0], &resource);
        let lower_inbound = lower.inbound(ids[0], ids[1], &resource);
        tokio::pin!(higher_inbound, lower_inbound);
        poll_fn(|cx| {
            assert!(higher_inbound.as_mut().poll(cx).is_pending());
            assert!(lower_inbound.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        assert!(!higher_outbound.cancellation.is_cancelled());
        session
            .outbound
            .send_replace(Some(higher_outbound.cancellation.clone()));
        poll_fn(|cx| {
            assert!(higher_inbound.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        assert!(
            higher_outbound.cancellation.is_cancelled(),
            "registration must wake the inbound waiter and break the two-handshake cycle"
        );
        assert!(!lower_outbound.cancellation.is_cancelled());
        drop(higher_outbound);
        let higher_inbound = higher_inbound.await;
        drop(lower_outbound);
        drop(higher_inbound);
        drop(lower_inbound.await);
    }

    #[tokio::test]
    async fn simultaneous_sessions_preempt_only_the_higher_id_initiator() {
        let sessions = DatabaseSessions::default();
        let mut ids = [
            iroh::SecretKey::generate().public(),
            iroh::SecretKey::generate().public(),
        ];
        ids.sort_by_key(|id| id.to_string());
        let resource = DatabaseResourceDescriptor {
            owner_subject: "owner".into(),
            application_id: "app".into(),
            database_id: "db".into(),
        };
        let outgoing = sessions.outbound(ids[0], &resource).await;
        let incoming = sessions.inbound(ids[1], ids[0], &resource);
        tokio::pin!(incoming);
        tokio::select! {
            biased;
            _ = &mut incoming => panic!("inbound must wait for cancelled outbound"),
            () = outgoing.cancellation.cancelled() => {}
        }
        drop(outgoing);
        drop(incoming.await);
        let outgoing = sessions.outbound(ids[1], &resource).await;
        let incoming = sessions.inbound(ids[0], ids[1], &resource);
        tokio::pin!(incoming);
        tokio::select! {
            biased;
            _ = &mut incoming => panic!("higher-ID inbound must queue behind the active lower-ID outbound"),
            () = std::future::ready(()) => {}
        }
        assert!(!outgoing.cancellation.is_cancelled());
        drop(outgoing);
        drop(incoming.await);
    }
}
