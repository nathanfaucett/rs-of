use std::{
    collections::{BTreeMap, BTreeSet, HashSet, VecDeque},
    fmt,
    io::{Error, ErrorKind},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use iroh::{
    Endpoint, EndpointAddr, EndpointId, RelayUrl, Watcher,
    endpoint::{Connection, VarInt},
    protocol::{AcceptError, ProtocolHandler},
};
use tokio::sync::{Mutex as AsyncMutex, broadcast};

use crate::store::EndpointIdStore;

#[cfg(not(wasm_browser))]
fn spawn<F: std::future::Future<Output = ()> + Send + 'static>(future: F) {
    tokio::spawn(future);
}

#[cfg(wasm_browser)]
fn spawn<F: std::future::Future<Output = ()> + 'static>(future: F) {
    wasm_bindgen_futures::spawn_local(future);
}

async fn sleep(duration: Duration) {
    #[cfg(not(wasm_browser))]
    tokio::time::sleep(duration).await;
    #[cfg(wasm_browser)]
    gloo_timers::future::TimeoutFuture::new(duration.as_millis() as u32).await;
}

async fn timeout<F: std::future::Future>(duration: Duration, future: F) -> Option<F::Output> {
    #[cfg(not(wasm_browser))]
    {
        tokio::time::timeout(duration, future).await.ok()
    }
    #[cfg(wasm_browser)]
    {
        use futures::future::{Either, select};
        let millis = duration.as_millis().min(u32::MAX as u128) as u32;
        let timer = Box::pin(gloo_timers::future::TimeoutFuture::new(millis));
        match select(Box::pin(future), timer).await {
            Either::Left((output, _)) => Some(output),
            Either::Right(((), _)) => None,
        }
    }
}

pub const MESH_ALPN: &[u8] = b"mesh/1";
pub const MAX_MESH_PEERS: usize = 6;
const MAX_PAYLOAD: usize = 64 * 1024;
const HEADER: usize = 101;
const MAX_FRAME: usize = HEADER + MAX_PAYLOAD;
const SEEN_LIMIT: usize = 4096;
const RETRY: Duration = Duration::from_secs(2);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const ON_DEMAND_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const PUBLISH_ATTEMPTS: u32 = 3;
const PUBLISH_BACKOFF: Duration = Duration::from_millis(500);

#[derive(Clone, Debug)]
pub struct Message {
    pub source: EndpointId,
    pub destination: Option<EndpointId>,
    pub payload: Vec<u8>,
}

#[derive(Clone)]
pub struct Mesh {
    inner: Arc<Inner>,
}

type Seen = (HashSet<[u8; 32]>, VecDeque<[u8; 32]>);

struct Inner {
    endpoint: Endpoint,
    peers: EndpointIdStore,
    connections: AsyncMutex<BTreeMap<EndpointId, (Connection, bool)>>,
    reconcile_lock: AsyncMutex<()>,
    seen: Mutex<Seen>,
    sequence: AtomicU64,
    started: AtomicBool,
    events: broadcast::Sender<Message>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        for (conn, _) in self.connections.get_mut().values() {
            conn.close(VarInt::from_u32(0), b"mesh stopped");
        }
    }
}

impl fmt::Debug for Mesh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mesh").finish_non_exhaustive()
    }
}

pub fn endpoint_relay_urls(endpoint: &Endpoint) -> Vec<RelayUrl> {
    let from_addr: Vec<RelayUrl> = endpoint.addr().relay_urls().cloned().collect();
    if !from_addr.is_empty() {
        return from_addr;
    }
    let mut status = endpoint.home_relay_status();
    status
        .get()
        .iter()
        .map(|relay| relay.url().clone())
        .collect()
}

fn neighbors(self_id: EndpointId, ids: Vec<EndpointId>) -> BTreeSet<EndpointId> {
    let mut ring: Vec<_> = ids.into_iter().chain([self_id]).collect();
    ring.sort_unstable();
    ring.dedup();
    let index = ring.binary_search(&self_id).expect("self is in ring");
    let mut result = BTreeSet::new();
    for distance in 1..=MAX_MESH_PEERS / 2 {
        result.insert(ring[(index + distance) % ring.len()]);
        result.insert(ring[(index + ring.len() - distance % ring.len()) % ring.len()]);
    }
    result.remove(&self_id);
    result
}

fn hop_limit(peers: usize) -> u32 {
    peers.saturating_add(1).min(u32::MAX as usize) as u32
}

fn frame(id: [u8; 32], message: &Message, hops: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER + message.payload.len());
    bytes.extend_from_slice(&id);
    bytes.extend_from_slice(message.source.as_bytes());
    bytes.push(u8::from(message.destination.is_some()));
    bytes.extend_from_slice(message.destination.unwrap_or(message.source).as_bytes());
    bytes.extend_from_slice(&hops.to_be_bytes());
    bytes.extend_from_slice(&message.payload);
    bytes
}

fn parse(bytes: &[u8]) -> Result<([u8; 32], Message, u32), Error> {
    if bytes.len() < HEADER || bytes.len() > MAX_FRAME || bytes[64] > 1 {
        return Err(Error::new(ErrorKind::InvalidData, "invalid mesh frame"));
    }
    let source = EndpointId::from_bytes(bytes[32..64].try_into().expect("fixed size"))
        .map_err(Error::other)?;
    let destination = if bytes[64] == 1 {
        Some(
            EndpointId::from_bytes(bytes[65..97].try_into().expect("fixed size"))
                .map_err(Error::other)?,
        )
    } else {
        None
    };
    Ok((
        bytes[..32].try_into().expect("fixed size"),
        Message {
            source,
            destination,
            payload: bytes[HEADER..].to_vec(),
        },
        u32::from_be_bytes(bytes[97..HEADER].try_into().expect("fixed size")),
    ))
}

impl Mesh {
    pub fn new(endpoint: Endpoint, peers: EndpointIdStore) -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(Inner {
                endpoint,
                peers,
                connections: AsyncMutex::new(BTreeMap::new()),
                reconcile_lock: AsyncMutex::new(()),
                seen: Mutex::new((HashSet::new(), VecDeque::new())),
                sequence: AtomicU64::new(0),
                started: AtomicBool::new(false),
                events,
            }),
        }
    }

    pub fn start(&self) {
        if self.inner.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let weak = Arc::downgrade(&self.inner);
        spawn(async move {
            loop {
                let Some(inner) = weak.upgrade() else { break };
                if inner.endpoint.is_closed() {
                    break;
                }
                let mesh = Mesh { inner };
                mesh.reconcile(false).await;
                drop(mesh);
                sleep(RETRY).await;
            }
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.inner.events.subscribe()
    }

    pub async fn broadcast(&self, payload: &[u8]) -> Result<(), Error> {
        self.publish(None, payload).await
    }

    pub async fn send(&self, destination: EndpointId, payload: &[u8]) -> Result<(), Error> {
        if !self.inner.peers.contains(destination) && destination != self.inner.endpoint.id() {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "mesh destination is not allowed",
            ));
        }
        self.publish(Some(destination), payload).await
    }

    async fn publish(&self, destination: Option<EndpointId>, payload: &[u8]) -> Result<(), Error> {
        if payload.len() > MAX_PAYLOAD {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "mesh payload is too large",
            ));
        }
        let source = self.inner.endpoint.id();
        let mut hash = blake3::Hasher::new();
        hash.update(source.as_bytes());
        hash.update(
            &self
                .inner
                .sequence
                .fetch_add(1, Ordering::Relaxed)
                .to_be_bytes(),
        );
        let id = *hash.finalize().as_bytes();
        self.remember(id);
        let message = Message {
            source,
            destination,
            payload: payload.to_vec(),
        };
        if destination.is_none() || destination == Some(source) {
            let _ = self.inner.events.send(message.clone());
        }
        if destination == Some(source) {
            return Ok(());
        }
        let hops = hop_limit(self.inner.peers.ids().len());
        let mut last_error = None;
        for attempt in 0..PUBLISH_ATTEMPTS {
            self.reconcile(true).await;
            match self.forward(id, &message, hops, None).await {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == ErrorKind::NotConnected => {
                    last_error = Some(error);
                    if attempt + 1 < PUBLISH_ATTEMPTS {
                        sleep(PUBLISH_BACKOFF).await;
                    }
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_error
            .unwrap_or_else(|| Error::new(ErrorKind::NotConnected, "no mesh neighbor connected")))
    }

    fn remember(&self, id: [u8; 32]) -> bool {
        let mut seen = self.inner.seen.lock().expect("mesh seen lock poisoned");
        if !seen.0.insert(id) {
            return false;
        }
        seen.1.push_back(id);
        if seen.1.len() > SEEN_LIMIT {
            let old = seen.1.pop_front().expect("cache not empty");
            seen.0.remove(&old);
        }
        true
    }

    async fn forward(
        &self,
        id: [u8; 32],
        message: &Message,
        hops: u32,
        incoming: Option<EndpointId>,
    ) -> Result<(), Error> {
        if hops == 0 {
            return Ok(());
        }
        let desired = neighbors(self.inner.endpoint.id(), self.inner.peers.ids());
        let connections: Vec<_> = self
            .inner
            .connections
            .lock()
            .await
            .iter()
            .filter(|(peer, (conn, _))| {
                desired.contains(*peer) && Some(**peer) != incoming && conn.close_reason().is_none()
            })
            .map(|(_, (conn, _))| conn.clone())
            .collect();
        if connections.is_empty() {
            return if incoming.is_none() && !desired.is_empty() {
                Err(Error::new(
                    ErrorKind::NotConnected,
                    "no mesh neighbor connected",
                ))
            } else {
                Ok(())
            };
        }
        let bytes = frame(id, message, hops);
        let mut delivered = false;
        let mut first_error = None;
        for conn in connections {
            let result = timeout(IO_TIMEOUT, async {
                let mut stream = conn.open_uni().await.map_err(Error::other)?;
                stream.write_all(&bytes).await.map_err(Error::other)?;
                stream.finish().map_err(Error::other)
            })
            .await
            .ok_or_else(|| Error::new(ErrorKind::TimedOut, "mesh send timed out"))
            .and_then(|result| result);
            match result {
                Ok(()) => delivered = true,
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if delivered {
            Ok(())
        } else {
            Err(first_error.unwrap_or_else(|| {
                Error::new(ErrorKind::NotConnected, "no mesh neighbor connected")
            }))
        }
    }

    async fn reconcile(&self, on_demand: bool) {
        let _reconcile = self.inner.reconcile_lock.lock().await;
        let self_id = self.inner.endpoint.id();
        let desired = neighbors(self_id, self.inner.peers.ids());
        {
            let mut connections = self.inner.connections.lock().await;
            connections.retain(|id, (conn, _)| {
                if !desired.contains(id)
                    || !self.inner.peers.contains(*id)
                    || conn.close_reason().is_some()
                {
                    conn.close(VarInt::from_u32(0), b"mesh topology changed");
                    false
                } else {
                    true
                }
            });
        }
        let relay_urls = endpoint_relay_urls(&self.inner.endpoint);
        for id in desired {
            if self.inner.endpoint.is_closed() {
                break;
            }
            if (!on_demand && id >= self_id)
                || !self.inner.peers.contains(id)
                || self.inner.connections.lock().await.contains_key(&id)
            {
                continue;
            }
            let mut addr = EndpointAddr::new(id);
            for url in &relay_urls {
                addr = addr.with_relay_url(url.clone());
            }
            let connect_timeout = if on_demand {
                ON_DEMAND_CONNECT_TIMEOUT
            } else {
                IO_TIMEOUT
            };
            if let Some(Ok(conn)) = timeout(
                connect_timeout,
                self.inner.endpoint.connect(addr, MESH_ALPN),
            )
            .await
                && self.register(conn.clone(), true).await
            {
                let weak = Arc::downgrade(&self.inner);
                spawn(async move { Mesh::run(weak, conn).await });
            }
        }
    }

    async fn register(&self, conn: Connection, outbound: bool) -> bool {
        let id = conn.remote_id();
        if self.inner.endpoint.is_closed()
            || !self.inner.peers.contains(id)
            || !neighbors(self.inner.endpoint.id(), self.inner.peers.ids()).contains(&id)
        {
            conn.close(VarInt::from_u32(0), b"mesh peer not allowed");
            return false;
        }
        let mut connections = self.inner.connections.lock().await;
        let preferred = (self.inner.endpoint.id() < id) == outbound;
        if let Some((existing, existing_outbound)) = connections.get(&id) {
            if !preferred || (self.inner.endpoint.id() < id) == *existing_outbound {
                conn.close(VarInt::from_u32(0), b"duplicate mesh connection");
                return false;
            }
            existing.close(VarInt::from_u32(0), b"replaced mesh connection");
        } else if connections.len() >= MAX_MESH_PEERS {
            conn.close(VarInt::from_u32(0), b"mesh full");
            return false;
        }
        connections.insert(id, (conn, outbound));
        true
    }

    async fn run(weak: Weak<Inner>, conn: Connection) {
        let peer = conn.remote_id();
        loop {
            #[cfg(not(wasm_browser))]
            let stream = tokio::select! {
                stream = conn.accept_uni() => match stream { Ok(stream) => stream, Err(_) => break },
                _ = conn.closed() => break,
            };
            #[cfg(wasm_browser)]
            let stream = {
                use futures::future::{Either, select};
                match select(Box::pin(conn.accept_uni()), Box::pin(conn.closed())).await {
                    Either::Left((Ok(stream), _)) => stream,
                    _ => break,
                }
            };
            let Some(inner) = weak.upgrade() else { break };
            let mesh = Mesh { inner };
            if mesh.inner.endpoint.is_closed()
                || !mesh.inner.peers.contains(peer)
                || !neighbors(mesh.inner.endpoint.id(), mesh.inner.peers.ids()).contains(&peer)
            {
                conn.close(VarInt::from_u32(0), b"mesh peer removed");
                break;
            }
            spawn(async move {
                let mut stream = stream;
                if let Some(Ok(bytes)) = timeout(IO_TIMEOUT, stream.read_to_end(MAX_FRAME)).await
                    && let Ok((id, message, hops)) = parse(&bytes)
                    && (message.source == peer || mesh.inner.peers.contains(message.source))
                    && message.destination.is_none_or(|dest| {
                        dest == mesh.inner.endpoint.id() || mesh.inner.peers.contains(dest)
                    })
                    && hops > 0
                    && hops <= hop_limit(mesh.inner.peers.ids().len())
                    && mesh.remember(id)
                {
                    if message.destination.is_none()
                        || message.destination == Some(mesh.inner.endpoint.id())
                    {
                        let _ = mesh.inner.events.send(message.clone());
                    }
                    if message.destination != Some(mesh.inner.endpoint.id()) {
                        let _ = mesh.forward(id, &message, hops - 1, Some(peer)).await;
                    }
                }
            });
        }
        if let Some(inner) = weak.upgrade() {
            let mut connections = inner.connections.lock().await;
            if connections
                .get(&peer)
                .is_some_and(|(current, _)| current.stable_id() == conn.stable_id())
            {
                connections.remove(&peer);
            }
        }
        conn.close(VarInt::from_u32(0), b"mesh connection ended");
    }
}

impl ProtocolHandler for Mesh {
    async fn accept(&self, conn: Connection) -> Result<(), AcceptError> {
        if !self.register(conn.clone(), false).await {
            conn.close(VarInt::from_u32(0), b"invalid mesh initiator");
            return Ok(());
        }
        Self::run(Arc::downgrade(&self.inner), conn).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iroh::{
        Endpoint, SecretKey, address_lookup::memory::MemoryLookup, endpoint::presets,
        protocol::Router,
    };

    use super::{MAX_MESH_PEERS, MESH_ALPN, Mesh, Message, frame, hop_limit, neighbors, parse};
    use crate::store::EndpointIdStore;

    #[test]
    fn ring_is_symmetric_and_bounded() {
        let ids: Vec<_> = (0..24).map(|_| SecretKey::generate().public()).collect();
        for id in &ids {
            let adjacent = neighbors(*id, ids.clone());
            assert_eq!(adjacent.len(), MAX_MESH_PEERS);
            for peer in adjacent {
                assert!(neighbors(peer, ids.clone()).contains(id));
            }
        }
        assert_eq!(neighbors(ids[0], ids[..3].to_vec()).len(), 2);
    }

    #[test]
    fn frame_round_trip_and_limits() {
        let source = SecretKey::generate().public();
        let destination = SecretKey::generate().public();
        let message = Message {
            source,
            destination: Some(destination),
            payload: vec![42; 16],
        };
        let data = frame([7; 32], &message, 4);
        let (id, decoded, hops) = parse(&data).unwrap();
        assert_eq!(id, [7; 32]);
        assert_eq!(hops, 4);
        assert_eq!(decoded.source, source);
        assert_eq!(decoded.destination, Some(destination));
        assert_eq!(decoded.payload, message.payload);
        assert!(parse(&data[..97]).is_err());
        assert!(parse(&[0; super::MAX_FRAME + 1]).is_err());
    }

    #[test]
    fn hop_limit_covers_large_rings() {
        assert_eq!(hop_limit(300), 301);
        assert_eq!(hop_limit(usize::MAX), u32::MAX);
        let source = SecretKey::generate().public();
        let message = Message {
            source,
            destination: None,
            payload: vec![],
        };
        assert_eq!(
            parse(&frame([1; 32], &message, hop_limit(300))).unwrap().2,
            301
        );
    }

    #[tokio::test]
    async fn first_send_dials_neighbor_without_background_reconcile() {
        let lookup = MemoryLookup::new();
        let first = Endpoint::builder(presets::Minimal)
            .alpns(vec![MESH_ALPN.to_vec()])
            .address_lookup(lookup.clone())
            .bind()
            .await
            .unwrap();
        let second = Endpoint::builder(presets::Minimal)
            .alpns(vec![MESH_ALPN.to_vec()])
            .address_lookup(lookup.clone())
            .bind()
            .await
            .unwrap();
        let (first, second) = if first.id() < second.id() {
            (first, second)
        } else {
            (second, first)
        };
        lookup.add_endpoint_info(first.addr());
        lookup.add_endpoint_info(second.addr());
        let peers = EndpointIdStore::new();
        peers.replace([first.id(), second.id()]);
        let sender = Mesh::new(first.clone(), peers.clone());
        let receiver = Mesh::new(second.clone(), peers);
        let mut subscription = receiver.subscribe();
        let first_router = Router::builder(first.clone())
            .accept(MESH_ALPN, sender.clone())
            .spawn();
        let second_router = Router::builder(second.clone())
            .accept(MESH_ALPN, receiver.clone())
            .spawn();
        tokio::time::timeout(Duration::from_secs(10), async {
            sender.send(second.id(), b"first").await.unwrap();
            let message = subscription.recv().await.unwrap();
            assert_eq!(message.source, first.id());
            assert_eq!(message.payload, b"first");
            sender.send(second.id(), b"second").await.unwrap();
            assert_eq!(subscription.recv().await.unwrap().payload, b"second");
            assert_eq!(sender.inner.connections.lock().await.len(), 1);
            assert_eq!(receiver.inner.connections.lock().await.len(), 1);
        })
        .await
        .unwrap();
        first.close().await;
        second.close().await;
        first_router.shutdown().await.unwrap();
        second_router.shutdown().await.unwrap();
    }
}
