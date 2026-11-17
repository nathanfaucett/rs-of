# Storage Sync Architecture

This document explains how database synchronization works in the Storage service, the components involved, and the current integration status.

## Overview

Storage provides two synchronization paths:

1. **Native mesh sync (Iroh)** — Peer-to-peer between approved Devices via `ofnet`/`iroh-chain`. Runs continuously in the background via `ChangeSubscription::run` workers. Used by native hosts.

2. **WebSocket sync (browser apps)** — Client-initiated WebSocket sessions to `wss://<storage-base>/sync/ws`. Carries the same ofdb SQL v7 owner protocol over a different transport. Used by browser applications.

This document covers **WebSocket sync** (path 2), which is the integration point for browser apps like password-manager.

## Component Map

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          BROWSER APP (e.g., password-manager)               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Local Database (@ofsdb/sql WASM)                                   │   │
│  │  - Database::open_indexeddb()                                       │   │
│  │  - Database::synchronize_checked(transport, config, role)           │   │
│  └─────────────────────────────┬──────────────────────────────────────┘   │
│                                │ SyncTransport trait                       │
│                                ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Sync Transport Adapter (MISSING)                                   │   │
│  │  - Implements ofdb_sql::SyncTransport                               │   │
│  │  - Delegates to SyncSession.send/receive                            │   │
│  └─────────────────────────────┬──────────────────────────────────────┘   │
│                                │                                           │
│                                ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  @ofsdk/storage-sync-client (npm)                                   │   │
│  │  - SyncClient.attach() -> SyncSession                               │   │
│  │  - SyncSession.send(bytes) / receive() -> bytes                     │   │
│  │  - Handles Hello/AttachRequest/AttachResponse handshake             │   │
│  │  - Frame encoding/decoding (version/flags/length/CRC32/Postcard)    │   │
│  └─────────────────────────────┬──────────────────────────────────────┘   │
│                                │ WebSocket (wss://)                        │
└────────────────────────────────┼──────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         STORAGE SERVER (storage-server)                     │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  WebSocketTransport (implements ofdb_sql::SyncTransport)            │   │
│  │  - Frame codec (encode/decode)                                      │   │
│  │  - Authorization via authorize_sync()                               │   │
│  └─────────────────────────────┬──────────────────────────────────────┘   │
│                                │                                           │
│                                ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  run_sync_session()                                                 │   │
│  │  - Opens selected database via owner API                            │   │
│  │  - Runs ofdb-sql sync engine (responder role)                       │   │
│  │  - Rechecks policy before reads/sends/apply                         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Wire Protocol

**Endpoint**: `wss://<storage-base>/sync/ws` (derived from HTTPS storage API URL)

**Framing**:

- Version: 1 byte (1)
- Flags: 1 byte (0)
- Payload length: u32 little-endian
- Payload: Postcard-encoded message
- CRC32: IEEE CRC32 of payload (little-endian)

**Handshake**:

1. Client -> Server: `Hello` (protocol_version=1, store_identity, supported_protocols=[OfdbSqlV7])
2. Server -> Client: `Hello` (echo)
3. Client -> Server: `AttachRequest` (store_identity, auth_token, direction, reconnect_token=None)
4. Server -> Client: `AttachResponse` (accepted, backend_replica_id, session_id, granted_direction, max_frame_bytes, max_session_bytes)

**Message Types**:

- `Hello` (0) — protocol identification
- `AttachRequest` (1) — attach with auth token
- `AttachResponse` (2) — attach result
- `OwnerMessage` (3) — opaque ofdb SQL v7 payload
- `Abort` (4) — connection termination with reason
- `Ping` (5) / `Pong` (6) — keepalive

## Authentication

**Token Audience**: `storage:sync` (NOT `storage:databases` or `storage:filesystems`)

**Token Acquisition**:

1. User signs in via IdP (OIDC) -> gets access token
2. Exchange via RFC 8693 token exchange:
   - `grant_type=urn:ietf:params:oauth:grant-type:token-exchange`
   - `subject_token=<user access token>`
   - `resource=storage:sync`
   - `authorization_details=[{type:"storage", actions:["sync"]}]`
3. Returns new access token with `storage:sync` audience

**Server Authorization** (`authorize_sync`):

- Validates token signature, expiry, audience
- Checks Device policy: both peers must select the resource
- Checks administrator restrictions
- Verifies resource kind/ID/namespace match
- Resolves endpoint via `local` lookup (currently hardcoded)
- Generates per-attach `backend_replica_id` (NOT read from database)

## StoreIdentity Requirements

```typescript
interface StoreIdentity {
  storeId: string; // Database UUID (stable)
  replicaId: string; // Browser/device instance UUID (stable per profile)
  creationEpoch: bigint; // Database creation timestamp (Unix ms)
  schemaFingerprint: Uint8Array; // 32-byte hash of schema (tables, columns, indexes)
}
```

**Critical**: Must be derived from the actual local database. Do not generate new `replicaId` per session. Do not use dummy fingerprints.

## Current Integration Status

| Component                                        | Status      | Location                                    |
| ------------------------------------------------ | ----------- | ------------------------------------------- |
| Server WebSocket endpoint                        | Complete    | `crates/storage-server/src/sync.rs`         |
| Server `WebSocketTransport`                      | Complete    | `crates/storage-server/src/sync.rs`         |
| `run_sync_session` (responder)                   | Complete    | `crates/storage-server/src/sync.rs`         |
| Rust WASM client (`BrowserSyncTransport`)        | Complete    | `crates/storage-sync-client/src/browser.rs` |
| Rust WASM client (`BrowserSyncClient.sync()`)    | Complete    | `crates/storage-sync-client/src/browser.rs` |
| npm raw transport (`@ofsdk/storage-sync-client`) | Complete    | `packages/storage-sync-client/src/`         |
| **Browser SyncTransport adapter (TS -> WASM)**   | **MISSING** | Browser sync integration                    |
| **WASM `ofdb-sql` binding for browser**          | **MISSING** | Browser sync integration                    |
| **`storage:sync` token flow in browser app**     | **MISSING** | Browser sync integration                    |
| **Stable `StoreIdentity` derivation**            | **MISSING** | Browser sync integration                    |

## What Works Today

1. **Native mesh sync** — Between native hosts via Iroh. Continuous, policy-driven.
2. **Server sync endpoint** — Accepts WebSocket connections, runs responder engine.
3. **Rust WASM client** — `BrowserSyncClient.sync()` can attach and run full sync IF you have `ofdb-sql` WASM loaded.
4. **Capability probing** — `@ofsdk/app` SyncPanel checks if service advertises sync, lists resources/devices.

## What Does NOT Work (Browser Sync Integration)

1. **No browser app can sync** — No app has the adapter + WASM engine + token flow wired together.
2. **`@ofsdk/storage-sync-client` is a raw pipe only** — It gives you `send/receive` bytes. You must build the `SyncTransport` adapter and call `synchronize_checked()` yourself.
3. **Direction contract incomplete** — Server always grants Bidirectional; Push/Pull enforcement not implemented.
4. **Hardcoded `local` endpoint** — Server doesn't resolve real Device selections for endpoint lookup.
5. **KV/filesystem sync over WebSocket** — Only SQL engine runs; no KV or file content transfer.

## Integration Path for Browser Apps

### Option A: Use Rust WASM Client (Recommended)

Add `storage-sync-client` as a WASM dependency, use `BrowserSyncClient.sync()` directly:

```rust
// In your WASM module
let client = BrowserSyncClient::new(url, token_provider, store_identity, local_replica_id).await?;
let result = client.sync(direction, &database).await?;
```

Pros: No adapter needed, `synchronize_checked()` called internally.
Cons: Requires WASM toolchain, larger bundle.

### Option B: Use npm Package + Build Adapter

```typescript
// 1. Get SyncSession from @ofsdk/storage-sync-client
const session = await client.attach({ identity, direction, signal });

// 2. Build SyncTransport adapter (YOU WRITE THIS)
class SyncTransportAdapter implements SyncTransport {
  constructor(private session: SyncSession) {}
  async receive(): Promise<Uint8Array> {
    return this.session.receive();
  }
  async send(payload: Uint8Array): Promise<void> {
    this.session.send(payload);
  }
}

// 3. Load ofdb-sql WASM and call synchronize_checked
const db = await Database.open_indexeddb("my-db");
const result = await db.synchronize_checked(
  adapter,
  config,
  SyncRole.Initiator,
);
```

Pros: Pure TypeScript, smaller if you already have WASM loader.
Cons: Must write adapter, must load ofdb-sql WASM separately.

## Key Files

| File                                           | Purpose                                                    |
| ---------------------------------------------- | ---------------------------------------------------------- |
| `crates/storage-server/src/sync.rs`            | Server WebSocket handler, authorization, responder         |
| `crates/storage-sync-client/src/browser.rs`    | Rust WASM client (BrowserSyncTransport, BrowserSyncClient) |
| `crates/storage-sync-client/src/client.rs`     | Native Rust client (WsSyncTransport, SyncClient)           |
| `crates/storage-sync-client/src/protocol.rs`   | Wire framing, message encoding/decoding                    |
| `packages/storage-sync-client/src/client.ts`   | npm SyncClient/SyncSession (raw byte pipe)                 |
| `packages/storage-sync-client/src/protocol.ts` | npm wire framing                                           |
| `ofdb/crates/sql-sync/src/session.rs`          | Owner sync engine (synchronize_checked)                    |
| `ofdb/crates/sql-sync/src/transport.rs`        | SyncTransport trait definition                             |
| `ofdb/crates/sql/src/database.rs`              | Database::synchronize_checked entry point                  |

## Related Documentation

- [Storage design](design.md) — Resource isolation, Device ownership, synchronization admission
- [Replication lifecycle](../storage-server/REPLICATION.md) — Native mesh sync lifecycle
- [System design](../../../docs/design.md) — Equal offline-first Nodes, local authorization
- [Public persistence](../../../docs/public-persistence.md) — Owner facade contracts, cleanup
- [Sync client README](../../packages/storage-sync-client/README.md) — npm package usage, integration details
