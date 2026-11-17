use std::{
    io::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use iroh::{
    Endpoint, EndpointAddr, EndpointId, SecretKey,
    endpoint::{Connection, presets::Preset},
    protocol::{AcceptError, ProtocolHandler, Router},
};
use noq::{RecvStream, SendStream};
use tokio::sync::{broadcast, watch};

use crate::{
    EndpointIdStore, PairingOffer,
    hooks::{AllowlistHook, AlpnAllowlist},
    mesh::{MESH_ALPN, Mesh, Message},
    pairing::validate_payload,
};

pub const PAIRING_ALPN: &[u8] = b"pairing/1";
pub const DATA_ALPN: &[u8] = b"data/1";

struct ServerInner {
    endpoint: Endpoint,
    allowed: EndpointIdStore,
    alpn_allowlist: AlpnAllowlist,
    mesh: Mesh,
    pairing_offers: broadcast::Sender<PairingOffer>,
    pairing_enabled: Arc<AtomicBool>,
    pairing_generation: AtomicU64,
    pairing_acceptance: watch::Sender<Option<u64>>,
    pairing_lock: Mutex<()>,
}

pub struct Server {
    inner: Arc<ServerInner>,
}

impl Clone for Server {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Server {
    pub fn new(endpoint: Endpoint, allowed: EndpointIdStore) -> Self {
        Self::from_parts(
            endpoint,
            allowed,
            Arc::new(AtomicBool::new(false)),
            AlpnAllowlist::default(),
        )
    }

    pub async fn bind<P>(
        preset: P,
        allowed: EndpointIdStore,
    ) -> Result<Self, iroh::endpoint::BindError>
    where
        P: Preset,
    {
        Self::bind_with_secret_key(preset, SecretKey::generate(), allowed).await
    }

    pub async fn bind_with_secret_key<P>(
        preset: P,
        secret_key: SecretKey,
        allowed: EndpointIdStore,
    ) -> Result<Self, iroh::endpoint::BindError>
    where
        P: Preset,
    {
        let pairing_enabled = Arc::new(AtomicBool::new(false));
        let alpn_allowlist = AlpnAllowlist::default();
        let endpoint = Endpoint::builder(preset)
            .secret_key(secret_key)
            .hooks(AllowlistHook::new(
                allowed.clone(),
                Arc::clone(&pairing_enabled),
                alpn_allowlist.clone(),
            ))
            .bind()
            .await?;
        Ok(Self::from_parts(
            endpoint,
            allowed,
            pairing_enabled,
            alpn_allowlist,
        ))
    }

    fn from_parts(
        endpoint: Endpoint,
        allowed: EndpointIdStore,
        pairing_enabled: Arc<AtomicBool>,
        alpn_allowlist: AlpnAllowlist,
    ) -> Self {
        endpoint.set_alpns(vec![
            DATA_ALPN.to_vec(),
            PAIRING_ALPN.to_vec(),
            MESH_ALPN.to_vec(),
        ]);
        let (pairing_offers, _) = broadcast::channel(64);
        let (pairing_acceptance, _) = watch::channel(None);
        let mesh = Mesh::new(endpoint.clone(), allowed.clone());
        Self {
            inner: Arc::new(ServerInner {
                endpoint,
                allowed,
                alpn_allowlist,
                mesh,
                pairing_offers,
                pairing_enabled,
                pairing_generation: AtomicU64::new(0),
                pairing_acceptance,
                pairing_lock: Mutex::new(()),
            }),
        }
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.inner.endpoint
    }

    pub fn allow_peer_for_alpn(&self, peer: EndpointId, alpn: &[u8], duration: Duration) {
        self.inner.alpn_allowlist.allow_for(peer, alpn, duration);
    }

    pub fn allowlist_hook(&self) -> AllowlistHook {
        AllowlistHook::new(
            self.inner.allowed.clone(),
            Arc::clone(&self.inner.pairing_enabled),
            self.inner.alpn_allowlist.clone(),
        )
    }

    pub fn router<D>(&self, data: D) -> Router
    where
        D: ProtocolHandler,
    {
        let router = Router::builder(self.inner.endpoint.clone())
            .accept(
                PAIRING_ALPN,
                PairingHandler {
                    server: self.clone(),
                },
            )
            .accept(DATA_ALPN, data)
            .accept(MESH_ALPN, self.inner.mesh.clone())
            .spawn();
        self.inner.mesh.start();
        router
    }

    pub fn router_with_protocol<D, H>(&self, data: D, alpn: &[u8], handler: H) -> Router
    where
        D: ProtocolHandler,
        H: ProtocolHandler,
    {
        let mut alpns = vec![
            DATA_ALPN.to_vec(),
            PAIRING_ALPN.to_vec(),
            MESH_ALPN.to_vec(),
        ];
        alpns.push(alpn.to_vec());
        self.inner.endpoint.set_alpns(alpns);
        let router = Router::builder(self.inner.endpoint.clone())
            .accept(
                PAIRING_ALPN,
                PairingHandler {
                    server: self.clone(),
                },
            )
            .accept(DATA_ALPN, data)
            .accept(MESH_ALPN, self.inner.mesh.clone())
            .accept(alpn, handler)
            .spawn();
        self.inner.mesh.start();
        router
    }

    pub fn subscribe_pairing_offers(&self) -> broadcast::Receiver<PairingOffer> {
        self.inner.pairing_offers.subscribe()
    }

    pub fn set_pairing_enabled(&self, enabled: bool) {
        let _lock = self
            .inner
            .pairing_lock
            .lock()
            .expect("pairing lock poisoned");
        let generation = self.inner.pairing_generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.inner.pairing_enabled.store(enabled, Ordering::Release);
        self.inner
            .pairing_acceptance
            .send_replace(enabled.then_some(generation));
    }

    pub fn set_pairing_enabled_for(&self, duration: Duration) {
        let generation = {
            let _lock = self
                .inner
                .pairing_lock
                .lock()
                .expect("pairing lock poisoned");
            let generation = self.inner.pairing_generation.fetch_add(1, Ordering::AcqRel) + 1;
            self.inner.pairing_enabled.store(true, Ordering::Release);
            self.inner.pairing_acceptance.send_replace(Some(generation));
            generation
        };
        let inner = Arc::clone(&self.inner);
        #[cfg(not(wasm_browser))]
        tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            let _lock = inner.pairing_lock.lock().expect("pairing lock poisoned");
            if inner.pairing_generation.load(Ordering::Acquire) == generation {
                inner.pairing_enabled.store(false, Ordering::Release);
                inner.pairing_acceptance.send_replace(None);
            }
        });
        #[cfg(wasm_browser)]
        wasm_bindgen_futures::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(duration.as_millis() as u32).await;
            let _lock = inner.pairing_lock.lock().expect("pairing lock poisoned");
            if inner.pairing_generation.load(Ordering::Acquire) == generation {
                inner.pairing_enabled.store(false, Ordering::Release);
                inner.pairing_acceptance.send_replace(None);
            }
        });
    }

    pub fn pairing_enabled(&self) -> bool {
        self.inner.pairing_enabled.load(Ordering::Acquire)
    }

    /// Identifies the current acceptance window; every enable/disable invalidates older offers.
    pub fn pairing_acceptance_generation(&self) -> Option<u64> {
        *self.inner.pairing_acceptance.borrow()
    }

    /// Observes acceptance changes, including natural expiration, without polling.
    pub fn subscribe_pairing_acceptance(&self) -> watch::Receiver<Option<u64>> {
        self.inner.pairing_acceptance.subscribe()
    }

    pub async fn is_allowed(&self, endpoint_id: EndpointId) -> bool {
        self.inner.allowed.contains(endpoint_id)
    }

    fn addr_with_relays(&self, addr: impl Into<EndpointAddr>) -> EndpointAddr {
        let mut addr = addr.into();
        for url in crate::mesh::endpoint_relay_urls(&self.inner.endpoint) {
            if !addr.relay_urls().any(|existing| existing == &url) {
                addr = addr.with_relay_url(url);
            }
        }
        addr
    }

    pub async fn send_pairing_offer(
        &self,
        endpoint: impl Into<EndpointAddr>,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        validate_payload(payload)?;
        let connection = self
            .inner
            .endpoint
            .connect(self.addr_with_relays(endpoint), PAIRING_ALPN)
            .await
            .map_err(Error::other)?;
        let (mut send, mut recv) = connection.open_bi().await?;
        send.write_all(payload).await.map_err(Error::other)?;
        send.finish().map_err(Error::other)?;
        recv.read_to_end(crate::MAX_PAIRING_PAYLOAD_LENGTH)
            .await
            .map_err(Error::other)
    }

    pub fn peers(&self) -> EndpointIdStore {
        self.inner.allowed.clone()
    }

    pub fn subscribe_messages(&self) -> broadcast::Receiver<Message> {
        self.inner.mesh.subscribe()
    }

    pub async fn broadcast(&self, payload: &[u8]) -> Result<(), Error> {
        self.inner.mesh.broadcast(payload).await
    }

    pub async fn send(&self, id: EndpointId, payload: &[u8]) -> Result<(), Error> {
        self.inner.mesh.send(id, payload).await
    }

    pub async fn connect_direct(&self, id: EndpointId) -> Result<Connection, Error> {
        self.connect_direct_with_alpn(id, DATA_ALPN).await
    }

    pub async fn connect_direct_with_alpn(
        &self,
        id: EndpointId,
        alpn: &[u8],
    ) -> Result<Connection, Error> {
        if !self.inner.allowed.contains(id) {
            return Err(Error::new(
                std::io::ErrorKind::PermissionDenied,
                "endpoint is not allowed",
            ));
        }
        self.inner
            .endpoint
            .connect(self.addr_with_relays(id), alpn)
            .await
            .map_err(Error::other)
    }

    pub async fn open_stream(
        &self,
        id: EndpointId,
    ) -> Result<(Connection, SendStream, RecvStream), Error> {
        let connection = self.connect_direct(id).await?;
        let (send, recv) = connection.open_bi().await?;
        Ok((connection, send, recv))
    }

    pub async fn close(&self) {
        self.set_pairing_enabled(false);
        self.inner.endpoint.close().await;
    }
}

#[derive(Clone)]
struct PairingHandler {
    server: Server,
}

impl std::fmt::Debug for PairingHandler {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingHandler")
            .finish_non_exhaustive()
    }
}

impl ProtocolHandler for PairingHandler {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        let remote_id = connection.remote_id();
        let generation = self.server.pairing_acceptance_generation();
        let (mut send, mut recv) = connection.accept_bi().await?;
        let payload = recv
            .read_to_end(crate::MAX_PAIRING_PAYLOAD_LENGTH)
            .await
            .map_err(Error::other)?;
        let Some(generation) = generation
            .filter(|generation| self.server.pairing_acceptance_generation() == Some(*generation))
        else {
            send.finish().map_err(Error::other)?;
            return Ok(());
        };
        self.server
            .inner
            .pairing_offers
            .send(PairingOffer::new(
                remote_id, payload, send, connection, generation,
            ))
            .map_err(|_| Error::other("pairing receiver closed"))?;
        Ok(())
    }
}
