# `ofnet` — endpoint and mesh networking

`ofnet` provides reusable Iroh networking for `of`. It owns Endpoint mechanics, peer pairing, allowlists, routing, mesh messages, and byte streams. It does not own Installation Devices, Users, Resources, or Resource authorization.

## Vocabulary

- **Endpoint:** a uniquely identified network participant.
- **Peer:** an Endpoint known as a possible mesh neighbor.
- **Mesh:** peer relationships, including relayed communication.
- **Broadcast / Direct Message:** message-level mesh delivery.
- **Byte Stream:** ordered bytes between two Endpoints without mesh message interpretation.

Pairing establishes a purpose-specific trust exchange. It is not Device approval or general Resource access. Allowlisting controls connections; it is not Resource Selection. Mesh forwarding does not provide transparent multihop SQL, KV, or filesystem synchronization.

See [target architecture](../docs/architecture.md#networking-and-synchronization).

## Crates

| Crate | Purpose |
| --- | --- |
| `iroh-chain` | Native endpoint, pairing, mesh, routing, and stream contracts |
| `iroh-chain-wasm` | Browser WebAssembly bindings |

The TypeScript wrapper is `packages/net`. It is a private workspace package, not a published public SDK.

## Development

```bash
cargo test -p iroh-chain --all-targets
pnpm --filter @ofsdk/net run build
```

Build from this checkout only. Browser WASM and native Iroh support are separate qualification paths.
