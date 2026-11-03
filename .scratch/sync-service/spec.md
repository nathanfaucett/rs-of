# Sync Service Spec

**Status:** `ready-for-agent`

## Context

Users want devices to act as storage nodes in a P2P mesh, with a Management UI to control which devices store which data. Apps (desktop, mobile, browser) must work offline-first and support pass-through files (video, images) that stream from online Full-residency peers.

Current architecture already provides:

- **Device mesh**: `storage-server` + `network` feature (iroh P2P) on each device
- **Selection API**: `device-server` `/devices/{id}/selection` endpoints
- **Offline storage**: `ofdb-sql` embedded (native/WASM)
- **Passthrough files**: `offs` `Residency::Passthrough` (metadata local, content streamed from Full peer)

**Missing piece**: How non-device apps (browser, mobile) connect to the mesh for sync.

## Problem

Apps cannot join the iroh P2P mesh directly (no iroh in browsers, complex on mobile). They need a WebSocket entry point to sync with the device mesh.

## Solution

Add a `sync` feature to `storage-server` that exposes a WebSocket endpoint at `/storage/sync/ws`. At least one device (or dedicated relay) enables this feature. Apps connect via WebSocket, authenticate with IdP token, and sync using the existing `ofdb-sql-sync` engine.

### Sync Flow

```
App (browser/mobile/desktop)
    │
    ▼ WebSocket /storage/sync/ws
Storage Server (any device with `sync` feature)
    │
    ├──► Validates IdP token (audience=storage, scope=sync)
    ├──► Checks Device Server: selected_resources for this app
    ├──► Starts ofdb-sql-sync session for authorized store
    └──► Proxies sync frames over WebSocket
```

### Passthrough Files

- App sets `Residency::Passthrough` per path
- Metadata syncs to all selected devices via normal sync
- Content fetched on-demand from any `Full` peer via iroh (device↔device) or WebSocket (app↔device)

## Scope

### In Scope

- `sync` feature on `storage-server` with WebSocket endpoint
- Auth integration: IdP token validation + Device Server policy check
- Bridge `ofdb-sql-sync` to WebSocket frames (reuse `sync-test-harness` protocol)
- SQL sync only (KV/filesystem sync later if needed)

### Out of Scope

- New service or central coordinator
- KV/filesystem sync over WebSocket (use SQL for now)
- Session resumption / reconnect tokens
- Metrics/health endpoints
- Dedicated relay deployment (deploy `sync` feature on any device)

## Architecture

### Components

| Component      | Location                        | Purpose                                       |
| -------------- | ------------------------------- | --------------------------------------------- |
| `sync` feature | `storage-server/Cargo.toml`     | Enables WebSocket sync endpoint               |
| `sync` module  | `storage-server/src/sync.rs`    | WebSocket handler, auth, session bridge       |
| Protocol types | Copied from `sync-test-harness` | `Hello`, `AttachRequest`, `SyncMessage`, etc. |

### Feature Flags

```toml
# storage-server/Cargo.toml
[features]
default = []
network = ["dep:axum", "dep:iroh", "dep:iroh-chain", "dep:tokio", "dep:tokio-tungstenite", "dep:tokio-util", "dep:ofdb-sql"]
sync = ["network", "dep:tungstenite", "dep:futures-util", "dep:postcard", "dep:serde", "dep:uuid"]
```

### Router Integration

```rust
// storage-server/src/router.rs
#[cfg(feature = "sync")]
let router = router.merge(sync_router(state, databases.clone(), endpoint));
```

## Seams (Test Points)

### 1. WebSocket Handler (Highest Seam)

- **Input**: WebSocket upgrade request, IdP token in `AttachRequest`
- **Output**: Sync frames proxied to/from `ofdb-sql-sync`
- **Test**: Connect with valid token → sync completes; invalid token → denied

### 2. Auth Function

```rust
async fn authorize_sync(
    idp_client: &IdpClient,
    device_client: &DeviceClient,
    token: &str,
    store_identity: &StoreIdentity,
    local_endpoint: EndpointId,
) -> Result<SyncAuth, SyncError>
```

- **Test**: Valid token + selected resource → OK; invalid token / unselected resource → Err

### 3. Session Bridge

```rust
struct SyncSession {
    sql_session: ofdb_sql_sync::SyncSession,
    ws_sink: WebSocketSink,
}
```

- **Test**: SQL session events → WebSocket frames; WebSocket frames → SQL session events

## Dependencies

- `ofdb-sql-sync` (existing, `continuous` feature)
- `sync-test-harness` protocol types (copied, not depended on)
- `idp-client` for token introspection
- `device-client` for `selected_resources` call
- `axum` + `tokio-tungstenite` for WebSocket

## Configuration

No new config. Uses existing:

- `RouterState` (IdP client, Device client, local endpoint)
- `DatabaseRuntime` (shared with P2P sync)

## Deployment

| Deployment             | Features           | Purpose                   |
| ---------------------- | ------------------ | ------------------------- |
| Device node (P2P only) | `network`          | Device↔Device replication |
| Sync entry point       | `sync`             | App↔Device WebSocket sync |
| Hybrid                 | `sync` + `network` | Both                      |

**Recommendation**: Enable `sync` on one always-on device (user's desktop/server). Add dedicated relay later if needed.

## Impact on Existing Apps & Packages

### `of/apps/app` (management UI)

- Device selection UI already exists: `DevicePolicy.svelte` uses `@aicacia/device-client` `PUT /devices/{id}/selection`
- Residency UI already exists: `ResourceResidency.svelte` uses `@aicacia/storage-client` `ResourcesResidencyApi`
- Token exchange pattern already in `StorageClient.exchangeToken()` — add `"sync"` resource for WS endpoint
- **Only new UI**: indicator showing which devices have `sync` feature enabled (can serve `/storage/sync/ws`)

### `of/apps/password-manager`

- Currently local-only: `@ofdb/sql` WASM SQLite + TanStack DB `createCollection` with manual sync hooks (`local-collection.ts`)
- **New adapter needed** in `@aicacia/storage-db-collection`: bridge TanStack DB sync → sync WebSocket (binary protocol)
- Vault encrypts locally (PBKDF2+AES-GCM) before storage — sync carries ciphertext only

### `of/packages/*`

- **No new OpenAPI client packages**: sync WS is binary (postcard+CRC32), not REST — not in OpenAPI specs
- `@aicacia/storage-client` `openSocket()` already demonstrates token-exchange → WebSocket upgrade pattern
- `@aicacia/storage-db-collection` already has `createWebSocketBackend` (KV over WS) and `createOfdbBackend` — add `createSyncBackend`

## Risks / Known Limits

- Protocol types duplicated from `sync-test-harness` (extract to shared crate later if needed)
- SQL sync only; KV/filesystem sync not included
- No session resumption (reconnect starts fresh)
- No metrics/health endpoint
- `ofdb-sql` WASM browser build must support sync engine (IndexedDB backend) — verify in ticket 05

## Acceptance Criteria

1. App connects to `wss://device.example.com/storage/sync/ws` with valid IdP token
2. App syncs SQL database with device mesh (offline edits converge)
3. Passthrough file reads stream content from Full peer
4. Device selection via Management UI controls which resources sync to which devices (UI already exists)
5. P2P device↔device sync continues working unchanged
6. Password manager syncs vault via new `storage-db-collection` sync adapter
7. No new `@aicacia/*-client` packages generated (sync WS is binary, outside OpenAPI)
