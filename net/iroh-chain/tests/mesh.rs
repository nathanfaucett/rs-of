use std::{io::Error, time::Duration};

use iroh::{
    Endpoint,
    address_lookup::MemoryLookup,
    endpoint::{Connection, presets},
    protocol::{AcceptError, ProtocolHandler},
};
use iroh_chain::{DATA_ALPN, EndpointIdStore, Server};

const TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone, Debug)]
struct Echo;

impl ProtocolHandler for Echo {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        while let Ok((mut send, mut recv)) = connection.accept_bi().await {
            send.write_all(b"peer-to-initiator: response")
                .await
                .map_err(Error::other)?;
            let bytes = recv.read_to_end(1024 * 1024).await.map_err(Error::other)?;
            if bytes != b"initiator-to-peer: distinct request" {
                return Err(AcceptError::from(Error::other("unexpected stream request")));
            }
            send.finish().map_err(Error::other)?;
        }
        Ok(())
    }
}

struct Network {
    servers: Vec<Server>,
    routers: Vec<iroh::protocol::Router>,
}

impl Network {
    async fn new(count: usize) -> Self {
        Self::new_with_excluded_member(count, None).await
    }

    async fn new_with_excluded_member(count: usize, excluded: Option<usize>) -> Self {
        let lookup = MemoryLookup::new();
        let mut endpoints = Vec::new();
        for _ in 0..count {
            let endpoint = Endpoint::builder(presets::Minimal)
                .address_lookup(lookup.clone())
                .bind()
                .await
                .unwrap();
            lookup.add_endpoint_info(endpoint.addr());
            endpoints.push(endpoint);
        }
        let ids: Vec<_> = endpoints.iter().map(Endpoint::id).collect();
        let mut servers = Vec::new();
        let mut routers = Vec::new();
        for (index, endpoint) in endpoints.into_iter().enumerate() {
            let peers = EndpointIdStore::new();
            peers.replace(ids.iter().enumerate().filter_map(|(peer_index, id)| {
                (peer_index != index && !(excluded == Some(peer_index) && index != peer_index))
                    .then_some(*id)
            }));
            let server = Server::new(endpoint, peers);
            routers.push(server.router(Echo));
            servers.push(server);
        }
        Self { servers, routers }
    }

    async fn close(self) {
        for server in &self.servers {
            server.close().await;
        }
        drop(self.routers);
    }
}

async fn receive(
    subscription: &mut tokio::sync::broadcast::Receiver<iroh_chain::Message>,
    scenario: &str,
) -> iroh_chain::Message {
    tokio::time::timeout(Duration::from_secs(20), subscription.recv())
        .await
        .unwrap_or_else(|_| panic!("{scenario}: message timed out"))
        .unwrap_or_else(|error| panic!("{scenario}: subscription failed: {error}"))
}

#[tokio::test]
async fn mesh_routes_broadcast_and_direct_messages_beyond_six_neighbors() {
    tokio::time::timeout(TIMEOUT, async {
        let network = Network::new(9).await;
        let ids: Vec<_> = network.servers.iter().map(|s| s.endpoint().id()).collect();
        let mut subscriptions: Vec<_> = network
            .servers
            .iter()
            .map(Server::subscribe_messages)
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        let (sender_index, target_index) = (0..ids.len())
            .flat_map(|sender| (0..ids.len()).map(move |target| (sender, target)))
            .find(|(sender, target)| {
                let sender_pos = sorted.iter().position(|id| id == &ids[*sender]).unwrap();
                let target_pos = sorted.iter().position(|id| id == &ids[*target]).unwrap();
                sender != target && {
                    let distance = sender_pos.abs_diff(target_pos);
                    distance.min(sorted.len() - distance) > 3
                }
            })
            .expect("nine peers must have non-neighbor pairs");
        let mut ready = vec![false; ids.len()];
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        while ready.iter().any(|received| !received) && tokio::time::Instant::now() < deadline {
            network.servers[sender_index]
                .broadcast(b"mesh-readiness-probe")
                .await
                .unwrap();
            for (index, subscription) in subscriptions.iter_mut().enumerate() {
                while let Ok(message) = subscription.try_recv() {
                    if message.payload == b"mesh-readiness-probe" {
                        ready[index] = true;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(
            ready.iter().all(|received| *received),
            "mesh did not converge: {ready:?}"
        );
        for subscription in &mut subscriptions {
            while subscription.try_recv().is_ok() {}
        }
        let broadcast = b"mesh-broadcast-nine-peers";
        network.servers[sender_index]
            .broadcast(broadcast)
            .await
            .unwrap();
        for (index, subscription) in subscriptions.iter_mut().enumerate() {
            let message = receive(subscription, &format!("broadcast receiver {index}")).await;
            assert_eq!(message.source, ids[sender_index]);
            assert_eq!(message.payload, broadcast);
            let deadline = tokio::time::Instant::now() + Duration::from_millis(150);
            while tokio::time::Instant::now() < deadline {
                if let Ok(duplicate) = tokio::time::timeout_at(deadline, subscription.recv()).await
                {
                    let duplicate = duplicate.expect("broadcast subscription closed");
                    assert_ne!(
                        duplicate.payload, broadcast,
                        "duplicate broadcast at {index}"
                    );
                }
            }
        }
        let direct = b"multi-hop-private-message";
        // `send` reports successful forwarding, not end-to-end acknowledgement.
        network.servers[sender_index]
            .send(ids[target_index], direct)
            .await
            .unwrap();
        let message = receive(&mut subscriptions[target_index], "multi-hop direct message").await;
        assert_eq!(message.source, ids[sender_index]);
        assert_eq!(message.destination, Some(ids[target_index]));
        assert_eq!(message.payload, direct);
        for (index, subscription) in subscriptions.iter_mut().enumerate() {
            if index == target_index {
                continue;
            }
            let deadline = tokio::time::Instant::now() + Duration::from_millis(250);
            while tokio::time::Instant::now() < deadline {
                if let Ok(Ok(other)) = tokio::time::timeout_at(deadline, subscription.recv()).await
                {
                    assert_ne!(
                        other.payload, direct,
                        "direct message leaked to endpoint {index}"
                    );
                }
            }
        }
        network.close().await;
    })
    .await
    .expect("multi-hop message test timed out");
}

#[tokio::test]
async fn router_with_protocol_advertises_and_routes_extra_and_existing_alpns() {
    const EXTRA_ALPN: &[u8] = b"caller-protocol/1";
    tokio::time::timeout(TIMEOUT, async {
        let lookup = MemoryLookup::new();
        let first_endpoint = Endpoint::builder(presets::Minimal)
            .address_lookup(lookup.clone())
            .bind()
            .await
            .unwrap();
        let second_endpoint = Endpoint::builder(presets::Minimal)
            .address_lookup(lookup.clone())
            .bind()
            .await
            .unwrap();
        lookup.add_endpoint_info(first_endpoint.addr());
        lookup.add_endpoint_info(second_endpoint.addr());
        let first_id = first_endpoint.id();
        let second_id = second_endpoint.id();
        let first_peers = EndpointIdStore::new();
        first_peers.add(second_id);
        let second_peers = EndpointIdStore::new();
        second_peers.add(first_id);
        let first_server = Server::new(first_endpoint, first_peers);
        let second_server = Server::new(second_endpoint, second_peers);
        let first_router = first_server.router(Echo);
        let second_router = second_server.router_with_protocol(Echo, EXTRA_ALPN, Echo);
        for alpn in [DATA_ALPN, EXTRA_ALPN] {
            let connection = first_server
                .connect_direct_with_alpn(second_id, alpn)
                .await
                .unwrap();
            let (mut send, mut recv) = connection.open_bi().await.unwrap();
            send.write_all(b"initiator-to-peer: distinct request")
                .await
                .unwrap();
            send.finish().unwrap();
            assert_eq!(
                recv.read_to_end(1024).await.unwrap(),
                b"peer-to-initiator: response"
            );
        }
        first_server.close().await;
        second_server.close().await;
        drop((first_router, second_router));
    })
    .await
    .expect("custom protocol routing test timed out");
}

#[tokio::test]
async fn direct_stream_exchanges_bytes_both_ways() {
    tokio::time::timeout(TIMEOUT, async {
        let network = Network::new(2).await;
        for alpn in [iroh_chain::DATA_ALPN] {
            let connection = network.servers[0]
                .connect_direct_with_alpn(network.servers[1].endpoint().id(), alpn)
                .await
                .unwrap();
            for _ in 0..2 {
                let (mut send, mut recv) = connection.open_bi().await.unwrap();
                send.write_all(b"initiator-to-peer: distinct request")
                    .await
                    .unwrap();
                send.finish().unwrap();
                assert_eq!(
                    recv.read_to_end(1024).await.unwrap(),
                    b"peer-to-initiator: response"
                );
            }
        }
        network.close().await;
    })
    .await
    .expect("two-way direct stream test timed out");
}

#[tokio::test]
async fn mesh_tracks_allowlist_changes_and_offline_peers() {
    tokio::time::timeout(TIMEOUT, async {
        let network = Network::new_with_excluded_member(3, Some(2)).await;
        let ids: Vec<_> = network.servers.iter().map(|s| s.endpoint().id()).collect();
        let mut subscriptions: Vec<_> = network
            .servers
            .iter()
            .map(Server::subscribe_messages)
            .collect();
        let added_id = ids[2];
        for server in &network.servers[..2] {
            assert!(!server.peers().contains(added_id));
            server.peers().add(added_id);
        }
        let payload = b"member-joined-direct";
        network.servers[0].send(added_id, payload).await.unwrap();
        let joined = receive(&mut subscriptions[2], "new member direct receive").await;
        assert_eq!(joined.payload, payload);
        network.servers[0]
            .broadcast(b"member-joined-broadcast")
            .await
            .unwrap();
        let joined = receive(&mut subscriptions[2], "new member broadcast receive").await;
        assert_eq!(joined.payload, b"member-joined-broadcast");

        for server in &network.servers[..2] {
            server.peers().remove(added_id);
            assert!(server.open_stream(added_id).await.is_err());
        }
        let mut removed_observer = network.servers[2].subscribe_messages();
        let mut survivor_observer = network.servers[1].subscribe_messages();
        let convergence_deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let mut consecutive_misses = 0;
        let mut probe_number = 0;
        while consecutive_misses < 3 && tokio::time::Instant::now() < convergence_deadline {
            let probe = format!("removed-peer-probe-{probe_number}");
            probe_number += 1;
            network.servers[0]
                .broadcast(probe.as_bytes())
                .await
                .unwrap();
            let survivor = receive(&mut survivor_observer, "broadcast after removal").await;
            assert_eq!(survivor.payload, probe.as_bytes());
            let probe_deadline = tokio::time::Instant::now() + Duration::from_millis(500);
            let delivered = loop {
                match tokio::time::timeout_at(probe_deadline, removed_observer.recv()).await {
                    Ok(Ok(message)) if message.payload == probe.as_bytes() => break true,
                    Ok(Ok(_)) => continue,
                    Ok(Err(_)) => break false,
                    Err(_) => break false,
                }
            };
            consecutive_misses = if delivered { 0 } else { consecutive_misses + 1 };
        }
        assert_eq!(
            consecutive_misses, 3,
            "removed peer did not converge out of mesh"
        );
        for subscription in &mut subscriptions {
            while subscription.try_recv().is_ok() {}
        }

        // An offline peer is distinct from removing a live endpoint from the allowlist.
        network.servers[2].close().await;
        let survivors = b"survivors-still-mesh";
        network.servers[0].broadcast(survivors).await.unwrap();
        for (index, subscription) in subscriptions[..2].iter_mut().enumerate() {
            let message = receive(subscription, &format!("surviving peer {index}")).await;
            assert_eq!(message.payload, survivors);
        }
        network.close().await;
    })
    .await
    .expect("membership test timed out");
}

#[tokio::test]
async fn acceptance_generations_change_on_reopen_and_publish_natural_expiry() {
    let server = Server::bind(presets::Minimal, EndpointIdStore::new())
        .await
        .expect("isolated acceptance server");
    let mut acceptance = server.subscribe_pairing_acceptance();
    assert_eq!(*acceptance.borrow(), None);
    server.set_pairing_enabled(true);
    let original = server
        .pairing_acceptance_generation()
        .expect("first acceptance window");
    assert_eq!(*acceptance.borrow(), Some(original));
    server.set_pairing_enabled(false);
    server.set_pairing_enabled(true);
    assert_ne!(server.pairing_acceptance_generation(), Some(original));
    server.set_pairing_enabled_for(Duration::from_millis(10));
    assert!(server.pairing_acceptance_generation().is_some());
    tokio::time::timeout(Duration::from_secs(5), acceptance.wait_for(Option::is_none))
        .await
        .expect("existing window expiry notification")
        .expect("live acceptance publisher");
    assert!(!server.pairing_enabled());
    assert_eq!(server.pairing_acceptance_generation(), None);
    server.close().await;
}

#[tokio::test]
async fn pairing_offer_and_reply_exchange() {
    tokio::time::timeout(TIMEOUT, async {
        let receiver = Server::bind(presets::Minimal, EndpointIdStore::new())
            .await
            .unwrap();
        let initiator = Server::bind(presets::Minimal, EndpointIdStore::new())
            .await
            .unwrap();
        let _receiver_router = receiver.router(Echo);
        let _initiator_router = initiator.router(Echo);
        initiator.peers().add(receiver.endpoint().id());
        let rejected = initiator
            .endpoint()
            .connect(receiver.endpoint().addr(), iroh_chain::MESH_ALPN)
            .await
            .expect("outbound QUIC connect should complete before remote hook rejects it");
        tokio::time::timeout(Duration::from_secs(5), rejected.closed())
            .await
            .expect("unknown peer connection was not rejected by the receiver hook");
        assert!(
            rejected.close_reason().is_some(),
            "rejected connection stayed open"
        );
        let mut offers = receiver.subscribe_pairing_offers();
        receiver.set_pairing_enabled(true);
        let initiator_client = initiator.clone();
        let address = receiver.endpoint().addr();
        let sending = tokio::spawn(async move {
            initiator_client
                .send_pairing_offer(address, b"pair me")
                .await
        });
        let offer = tokio::time::timeout(Duration::from_secs(10), offers.recv())
            .await
            .expect("pairing offer timed out")
            .unwrap();
        assert_eq!(offer.remote_id, initiator.endpoint().id());
        assert_eq!(offer.payload, b"pair me");
        assert_eq!(
            Some(offer.acceptance_generation),
            receiver.pairing_acceptance_generation()
        );
        assert!(!offer.is_closed());
        offer.reply(b"paired").await.unwrap();
        assert_eq!(sending.await.unwrap().unwrap(), b"paired");

        receiver.peers().add(initiator.endpoint().id());
        let mut messages = receiver.subscribe_messages();
        initiator
            .send(receiver.endpoint().id(), b"pairing-complete")
            .await
            .unwrap();
        let message = receive(&mut messages, "message after pairing").await;
        assert_eq!(message.source, initiator.endpoint().id());
        assert_eq!(message.payload, b"pairing-complete");
        receiver.set_pairing_enabled(false);
        assert!(!receiver.pairing_enabled());
        let unknown = Server::bind(presets::Minimal, EndpointIdStore::new())
            .await
            .unwrap();
        let _unknown_router = unknown.router(Echo);
        unknown.peers().add(receiver.endpoint().id());
        assert!(
            unknown
                .send_pairing_offer(receiver.endpoint().addr(), b"pairing disabled")
                .await
                .is_err()
        );
        receiver.close().await;
        tokio::time::timeout(Duration::from_secs(5), offer.closed())
            .await
            .expect("answered pairing offer observes transport closure");
        assert!(offer.is_closed());
        initiator.close().await;
        unknown.close().await;
    })
    .await
    .expect("pairing test timed out");
}
