# iroh-chain — Reusable Iroh Mesh Networking

`iroh-chain` provides reusable Iroh endpoints, mesh/pairing mechanics, endpoint allowlist enforcement, and generic connections/routing for the `of` ecosystem.

## Features

- **Endpoint management** — Single Iroh endpoint per Node, persisted and restored
- **Mesh messaging** — Broadcast to all peers or direct messages with hop-limited forwarding
- **Pairing** — Challenge/response handshake (Ed25519) to establish trust between endpoints
- **Allowlist hooks** — Control which peers and protocols can connect
- **Byte streams** — Ordered flow of bytes between two endpoints
- **WASM support** — Full browser build via `iroh-chain-wasm`

## Quick Start

```rust
use iroh_chain::{Server, PairingOffer, EndpointId};
use iroh::Endpoint;

// Create server with persisted endpoint
let server = Server::builder()
    .with_endpoint(endpoint)
    .build()
    .await?;

// Get node address for sharing
let node_addr = server.endpoint().node_addr().await?;

// Accept incoming connections
server.accept().await?;

// Pair with another device
let offer = PairingOffer::new(
    "my-installation",
    server.endpoint().id(),
    "My Device"
);
let approval = server.pair_with(trusted_peer, &offer).await?;
```

## Mesh Messaging

```rust
use iroh_chain::{Server, Message};

// Broadcast to all mesh peers
server.broadcast(Message::new(b"hello mesh")).await?;

// Direct message to specific peer
server.send_direct(target_peer, Message::new(b"private")).await?;

// Subscribe to incoming
let mut rx = server.subscribe();
while let Some(msg) = rx.recv().await {
    println!("Received from {}: {:?}", msg.from, msg.payload);
}
```

## Byte Streams

```rust
use iroh_chain::{Server, Stream};

// Open bidirectional byte stream
let (mut send, mut recv) = server.open_stream(peer_id).await?;

// Send data
send.write_all(b"hello stream").await?;

// Receive data
let mut buf = vec![0; 1024];
let n = recv.read(&mut buf).await?;
```

## Allowlist

```rust
use iroh_chain::{Server, AllowlistDecision};

server.set_allowlist(move |peer_id, protocol| {
    // Allow only known peers for mesh protocol
    if protocol == "of/mesh" {
        AllowlistDecision::Allow
    } else {
        AllowlistDecision::Deny
    }
});
```

## WASM

Build for browser:

```bash
cd net/iroh-chain-wasm
wasm-pack build --target web
```

The generated `pkg/` is checked into `packages/net/` for npm publishing as `@ofsdk/net`.

## Integration with `of`

`of` owns domain identity, policy, catalogs, endpoint-to-Installation-Device trust, local authorization, and protocol-specific transport adapters wiring persistence owners.

- `iroh-chain` owns reusable Iroh endpoints, mesh/pairing mechanics, endpoint allowlist enforcement, and generic connections/routing.
- Each Node shares one `iroh-chain` endpoint/router, with endpoint identity and locally trusted membership persisted and restored by `of`.
- Allowlisting is not resource authorization; pairing is not general access.
- Mesh messages and direct streams are distinct, with no transparent multihop database/filesystem sync claim.

See [of networking composition and interfaces](../docs/design.md#networking-composition-and-trust).

## Testing

```bash
# Native
cargo test -p iroh-chain --all-targets

# WASM
cd net/iroh-chain-wasm
wasm-pack test --headless --firefox
```

## Related

- [net/README.md](../README.md) — ofnet overview
- [db/README.md](../../db/README.md) — Offline-first databases
- [fs/README.md](../../fs/README.md) — Offline-first filesystem
- [packages/README.md](../../packages/README.md) — TypeScript SDKs
