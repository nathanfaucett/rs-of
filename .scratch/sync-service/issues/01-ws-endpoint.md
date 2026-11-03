# 01: WebSocket Endpoint Responds

**Status:** `ready-for-agent`
**Type:** `task`
**Blocked by:** none

## Description

Add `sync` feature to `storage-server` with a minimal WebSocket endpoint at `/storage/sync/ws` that accepts connections and echoes messages.

## What it delivers

- `cargo build --package storage-server --features sync,network` compiles
- `ws://localhost:3000/storage/sync/ws` accepts WebSocket upgrade
- Server echoes binary frames back to client
- No auth, no sync logic — just transport works

## Scope

- `storage-server/Cargo.toml`: add `sync` feature with `tungstenite`, `tokio-tungstenite`, `futures-util`
- `storage-server/src/sync.rs`: minimal `sync_handler` that echoes frames
- `storage-server/src/router.rs`: mount `/sync/ws` route under `#[cfg(feature = "sync")]`
- Pass `EndpointId` in state (needed for later auth)

## Acceptance Criteria

- [ ] Build succeeds with `sync` feature
- [ ] `websocat ws://localhost:3000/storage/sync/ws` connects
- [ ] Binary message sent → same binary message received back
- [ ] Connection closes cleanly

## Notes

- Copy `Hello`/`SyncMessage` types from `sync-test-harness` for later use
- Use existing `LocalTls` for TLS (wss://)
- No IdP/Device client wiring yet
