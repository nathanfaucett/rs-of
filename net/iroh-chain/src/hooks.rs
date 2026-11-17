use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[cfg(not(wasm_browser))]
use std::time::Instant;
#[cfg(wasm_browser)]
use web_time::Instant;

use iroh::endpoint::{
    AfterHandshakeOutcome, BeforeConnectOutcome, Connection, EndpointHooks, VarInt,
};

use crate::{EndpointIdStore, PAIRING_ALPN};

type TemporaryPeers = BTreeMap<(iroh::EndpointId, Vec<u8>), Instant>;

#[derive(Clone, Default)]
pub struct AlpnAllowlist {
    peers: Arc<Mutex<TemporaryPeers>>,
}

impl AlpnAllowlist {
    pub fn allow_for(&self, peer: iroh::EndpointId, alpn: &[u8], duration: Duration) {
        let mut peers = self.peers.lock().expect("ALPN allowlist lock poisoned");
        peers.retain(|_, expires_at| *expires_at > Instant::now());
        peers.insert((peer, alpn.to_vec()), Instant::now() + duration);
    }

    fn contains(&self, peer: iroh::EndpointId, alpn: &[u8]) -> bool {
        let mut peers = self.peers.lock().expect("ALPN allowlist lock poisoned");
        peers.retain(|_, expires_at| *expires_at > Instant::now());
        peers.contains_key(&(peer, alpn.to_vec()))
    }
}

pub struct AllowlistHook {
    allowed: EndpointIdStore,
    pairing_enabled: Arc<AtomicBool>,
    alpn_allowlist: AlpnAllowlist,
}

impl AllowlistHook {
    pub fn new(
        allowed: EndpointIdStore,
        pairing_enabled: Arc<AtomicBool>,
        alpn_allowlist: AlpnAllowlist,
    ) -> Self {
        Self {
            allowed,
            pairing_enabled,
            alpn_allowlist,
        }
    }
}

impl std::fmt::Debug for AllowlistHook {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AllowlistHook")
            .finish_non_exhaustive()
    }
}

impl EndpointHooks for AllowlistHook {
    async fn before_connect<'a>(
        &'a self,
        remote_addr: &'a iroh::EndpointAddr,
        alpn: &'a [u8],
    ) -> BeforeConnectOutcome {
        if self.allowed.contains(remote_addr.id)
            || self.alpn_allowlist.contains(remote_addr.id, alpn)
            || (alpn == PAIRING_ALPN && self.pairing_enabled.load(Ordering::Acquire))
        {
            BeforeConnectOutcome::Accept
        } else {
            BeforeConnectOutcome::Reject
        }
    }

    async fn after_handshake<'a>(&'a self, conn: &'a Connection) -> AfterHandshakeOutcome {
        let allowed = self.allowed.contains(conn.remote_id());
        let pairing = conn.alpn() == PAIRING_ALPN && self.pairing_enabled.load(Ordering::Acquire);
        let alpn_allowed = self.alpn_allowlist.contains(conn.remote_id(), conn.alpn());
        if allowed || pairing || alpn_allowed {
            AfterHandshakeOutcome::Accept
        } else {
            AfterHandshakeOutcome::Reject {
                error_code: VarInt::from_u32(0x100),
                reason: b"endpoint is not allowlisted".to_vec(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        sync::{Arc, atomic::AtomicBool},
    };

    use iroh::{
        EndpointAddr, SecretKey,
        endpoint::{BeforeConnectOutcome, EndpointHooks},
    };

    use super::{AllowlistHook, AlpnAllowlist};
    use crate::{EndpointIdStore, PAIRING_ALPN};

    #[tokio::test]
    async fn allows_only_trusted_or_open_pairing_connections() {
        let allowed = EndpointIdStore::new();
        let trusted = SecretKey::generate().public();
        let untrusted = SecretKey::generate().public();
        allowed.replace([trusted]);
        let pairing_enabled = Arc::new(AtomicBool::new(false));
        let hook = AllowlistHook::new(
            allowed,
            Arc::clone(&pairing_enabled),
            AlpnAllowlist::default(),
        );
        let trusted_addr = EndpointAddr {
            id: trusted,
            addrs: BTreeSet::new(),
        };
        let untrusted_addr = EndpointAddr {
            id: untrusted,
            addrs: BTreeSet::new(),
        };

        assert!(matches!(
            hook.before_connect(&trusted_addr, crate::DATA_ALPN).await,
            BeforeConnectOutcome::Accept
        ));
        assert!(matches!(
            hook.before_connect(&untrusted_addr, crate::DATA_ALPN).await,
            BeforeConnectOutcome::Reject
        ));
        assert!(matches!(
            hook.before_connect(&untrusted_addr, PAIRING_ALPN).await,
            BeforeConnectOutcome::Reject
        ));

        pairing_enabled.store(true, std::sync::atomic::Ordering::Release);
        assert!(matches!(
            hook.before_connect(&untrusted_addr, PAIRING_ALPN).await,
            BeforeConnectOutcome::Accept
        ));

        let bootstrap_alpn = b"bootstrap/1";
        let alpn_allowlist = AlpnAllowlist::default();
        alpn_allowlist.allow_for(untrusted, bootstrap_alpn, std::time::Duration::from_secs(1));
        let hook = AllowlistHook::new(
            EndpointIdStore::new(),
            Arc::new(AtomicBool::new(false)),
            alpn_allowlist,
        );
        assert!(matches!(
            hook.before_connect(&untrusted_addr, bootstrap_alpn).await,
            BeforeConnectOutcome::Accept
        ));
        assert!(matches!(
            hook.before_connect(&untrusted_addr, crate::DATA_ALPN).await,
            BeforeConnectOutcome::Reject
        ));
    }
}
