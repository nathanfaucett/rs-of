# @ofsdk/net — Mesh Networking SDK

TypeScript/JavaScript SDK for `iroh-chain` mesh networking, built from `net/iroh-chain-wasm` via `wasm-pack`.

## Installation

```bash
npm install @ofsdk/net
# or
pnpm add @ofsdk/net
```

## Usage

```typescript
import { createNetClient, Message, EndpointId } from '@ofsdk/net';

// Create client
const net = await createNetClient();

// Connect to trusted peer
await net.connect(trustedPeerNodeAddr);

// Send direct message
await net.sendDirect(peerId, new Message(new TextEncoder().encode("hello")));

// Broadcast to mesh
await net.broadcast(new Message(new TextEncoder().encode("hello mesh")));

// Subscribe to messages
net.onMessage((msg) => {
  console.log(`Received from ${msg.from}:`, new TextDecoder().decode(msg.payload));
});

// Open byte stream
const stream = await net.openStream(peerId);
const writer = stream.writable.getWriter();
await writer.write(new TextEncoder().encode("stream data"));
await writer.close();

// Pair with another device
const approval = await net.pairWith(trustedPeer, {
  installation: "my-installation",
  deviceName: "My Device"
});
```

## API

### `createNetClient(options?)`

Creates a new mesh networking client.

Options:
- `memory?: boolean` — Use in-memory storage (for testing)
- `dataDir?: string` — Persistent data directory

### `NetClient`

| Method | Description |
|--------|-------------|
| `connect(nodeAddr: string)` | Connect to a peer via node address |
| `sendDirect(peerId: EndpointId, message: Message)` | Send direct message to peer |
| `broadcast(message: Message)` | Broadcast message to all mesh peers |
| `onMessage(handler: (msg: Message) => void)` | Subscribe to incoming messages |
| `openStream(peerId: EndpointId)` | Open byte stream to peer |
| `pairWith(peerId: EndpointId, offer: PairingOffer)` | Pair with another device |
| `getEndpointId()` | Get local endpoint ID |
| `getNodeAddr()` | Get local node address for sharing |

### Types

```typescript
interface Message {
  from: EndpointId;
  payload: Uint8Array;
  timestamp: number;
}

interface EndpointId {
  toString(): string;
}

interface PairingOffer {
  installation: string;
  deviceName: string;
}

interface PairingApproval {
  request: PairingOffer;
  ownerSubject: EndpointId;
  signature: string;
}
```

## Building from Source

```bash
# Requires Rust toolchain (pinned in rust-toolchain.toml)
cd net/iroh-chain-wasm
wasm-pack build --target web --out-dir ../../packages/net/pkg
```

The `pkg/` directory is checked in for npm publishing.

## Related

- [net/iroh-chain/README.md](../../net/iroh-chain/README.md) — Rust crate documentation
- [net/README.md](../../net/README.md) — ofnet overview
- [packages/README.md](../README.md) — All TypeScript SDK packages
