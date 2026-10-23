use crate::security_protocol::BOOTSTRAP_ALPN;
use iroh::{
    Endpoint, EndpointAddr, EndpointId, SecretKey,
    endpoint::{
        AfterHandshakeOutcome, BeforeConnectOutcome, Connection, EndpointHooks, presets::Preset,
    },
};
use iroh_chain::{AllowlistHook, EndpointIdStore, PAIRING_ALPN, Server};
use std::{
    io,
    sync::{Arc, OnceLock},
};

/// Identity-owner verification of retained preparation, signer and (when required) approval.
pub trait RetainedJoinAdmission: Send + Sync + 'static {
    fn pinned_join_peer(
        &self,
        local_endpoint: EndpointId,
        approved: bool,
    ) -> io::Result<Option<EndpointId>>;
}

struct JoinHook<A> {
    ordinary: Arc<OnceLock<AllowlistHook>>,
    retained: A,
    local_endpoint: EndpointId,
}
impl<A> std::fmt::Debug for JoinHook<A> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JoinHook")
    }
}
impl<A: RetainedJoinAdmission> JoinHook<A> {
    fn pinned_peer_for(&self, alpn: &[u8]) -> io::Result<Option<EndpointId>> {
        if alpn != PAIRING_ALPN && alpn != BOOTSTRAP_ALPN {
            return Ok(None);
        }
        self.retained
            .pinned_join_peer(self.local_endpoint, alpn == BOOTSTRAP_ALPN)
    }
}
impl<A: RetainedJoinAdmission> EndpointHooks for JoinHook<A> {
    async fn before_connect<'a>(
        &'a self,
        remote: &'a EndpointAddr,
        alpn: &'a [u8],
    ) -> BeforeConnectOutcome {
        if self.pinned_peer_for(alpn).ok().flatten() == Some(remote.id) {
            return BeforeConnectOutcome::Accept;
        }
        match self.ordinary.get() {
            Some(hook) => hook.before_connect(remote, alpn).await,
            None => BeforeConnectOutcome::Reject,
        }
    }

    async fn after_handshake<'a>(&'a self, connection: &'a Connection) -> AfterHandshakeOutcome {
        if self.pinned_peer_for(connection.alpn()).ok().flatten() == Some(connection.remote_id()) {
            return AfterHandshakeOutcome::Accept;
        }
        match self.ordinary.get() {
            Some(hook) => hook.after_handshake(connection).await,
            None => AfterHandshakeOutcome::Reject {
                error_code: 1u32.into(),
                reason: b"local admission not initialized".to_vec(),
            },
        }
    }
}

pub async fn bind_node_join_endpoint<P: Preset, A: RetainedJoinAdmission>(
    preset: P,
    secret: SecretKey,
    allowed: EndpointIdStore,
    retained: A,
) -> io::Result<Server> {
    let ordinary = Arc::new(OnceLock::new());
    let endpoint = Endpoint::builder(preset)
        .secret_key(secret.clone())
        .hooks(JoinHook {
            ordinary: ordinary.clone(),
            retained,
            local_endpoint: secret.public(),
        })
        .bind()
        .await
        .map_err(io::Error::other)?;
    let server = Server::new(endpoint, allowed);
    ordinary
        .set(server.allowlist_hook())
        .map_err(|_| io::Error::other("local admission already initialized"))?;
    Ok(server)
}
