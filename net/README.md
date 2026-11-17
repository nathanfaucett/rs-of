# ofnet — Offline-First Mesh Networking

`ofnet` provides offline-first peer-to-peer mesh networking built on [iroh](https://iroh.computer).

Endpoints form a resilient mesh where messages reach peers directly or through relays — no central server required. Includes secure pairing for trusting new devices and works in both native Rust and the browser via WASM.

## Core Concepts

- **Endpoint** — A uniquely identified participant in the network that can communicate with other endpoints.
- **Peer** — An endpoint known to another endpoint as a possible mesh neighbor.
- **Mesh** — The network of peer relationships through which endpoints communicate, including communication relayed by intermediate endpoints.
- **Broadcast** — A message intended for every endpoint in the mesh.
- **Direct Message** — A message intended for one identified endpoint, whether reached directly or through relays.
- **Byte Stream** — An ordered flow of bytes between two endpoints, without message-level interpretation by the mesh.

## Crates

| Crate | Purpose |
|-------|---------|
| `iroh-chain` | Reusable Iroh endpoints, mesh/pairing mechanics, endpoint allowlist enforcement, generic connections/routing |
| `iroh-chain-wasm` | WASM build for browser (wasm-pack, own Cargo.lock) |

## Features

| Feature | Description |
|---------|-------------|
| `std` | Standard library (default) |
| `mesh` | Mesh messaging (broadcast, direct messages) |
| `pairing` | Challenge/response handshake for trust establishment |
| `allowlist` | Hooks to control which peers/protocols can connect |
| `wasm` | Browser build (wasm-bindgen, IndexedDB) |

## Quick Start

```rust
use iroh_chain::{Server, PairingOffer, EndpointId};
use iroh::Endpoint;

// Create a server (endpoint + router)
let server = Server::builder().build().await?;

// Get endpoint info
let endpoint_id = server.endpoint().id();
let node_addr = server.endpoint().node_addr().await?;

// Accept incoming connections
server.accept().await?;

// Pair with another device
let offer = PairingOffer::new(installation_id, endpoint_id, "My Device");
let approval = server.pair_with(trusted_peer, &offer).await?;
```

## Mesh Messaging

```rust
use iroh_chain::{Server, Message};

// Broadcast to all peers
server.broadcast(Message::new(b"hello mesh")).await?;

// Direct message to specific peer
server.send_direct(target_peer, Message::new(b"private")).await?;

// Subscribe to incoming messages
let mut rx = server.subscribe();
while let Some(msg) = rx.recv().await {
    println!("Received: {:?}", msg);
}
```

## Byte Streams

```rust
use iroh_chain::{Server, Stream};

// Open a byte stream to a peer
let (mut send, mut recv) = server.open_stream(peer_id).await?;

// Send data
send.write_all(b"hello stream").await?;

// Receive data
let mut buf = vec![0; 1024];
let n = recv.read(&mut buf).await?;
```

## WASM

The `iroh-chain-wasm` crate is a standalone `wasm-pack` workspace (own `Cargo.lock`). It is **excluded** from the Cargo workspace. Build with:

```bash
cd net/iroh-chain-wasm
wasm-pack build --target web
```

The generated `pkg/` is checked into `packages/net/` for npm publishing.

## Testing

```bash
# Native
cargo test -p iroh-chain --all-targets

# WASM (from net/iroh-chain-wasm)
wasm-pack test --headless --firefox
```

## Architecture

```
iroh-chain
  ├── endpoint (Iroh endpoint management)
  ├── router (protocol routing, ALPN)
  ├── mesh (broadcast, direct messages, hop-limited forwarding)
  ├── pairing (challenge/response, Ed25519 signatures)
  ├── allowlist (peer/protocol filtering hooks)
  └── streams (byte streams, bidirectional)
```

## Integration with `of`

`of` owns domain identity, policy, catalogs, endpoint-to-Installation-Device trust, local authorization, and protocol-specific transport adapters wiring persistence owners.

- `iroh-chain` owns reusable Iroh endpoints, mesh/pairing mechanics, endpoint allowlist enforcement, and generic connections/routing.
- Each Node shares one `iroh-chain` endpoint/router, with endpoint identity and locally trusted membership persisted and restored by `of`.
- Allowlisting is not resource authorization; pairing is not general access.
- Mesh messages and direct streams are distinct, with no transparent multihop database/filesystem sync claim.

See [of networking composition and interfaces](../docs/design.md#networking-composition-and-trust).

## Related

- [db/README.md](../db/README.md) — Offline-first databases (`ofdb-sql`, `ofdb-kv`)
- [fs/README.md](../fs/README.md) — Offline-first filesystem (`file-system`)
- [packages/README.md](../packages/README.md) — TypeScript SDK packages
