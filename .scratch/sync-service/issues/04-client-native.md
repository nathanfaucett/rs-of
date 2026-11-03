# 04: sync-client Native — Connect + Auth + Sync

**Status:** `ready-for-agent`
**Type:** `task`
**Blocked by:** 03

## Description

Create `sync-client` crate (native feature) that connects to server, authenticates, and runs sync against local `ofdb-sql` embedded database.

## What it delivers

- `SyncClient::new(url, token_provider).sync(store_identity, direction).await` works
- Native binary connects to `wss://device.example.com/storage/sync/ws`
- Token provider callback supplies valid IdP token
- Local `ofdb-sql` embedded database syncs with server

## Scope

- New crate: `of/crates/sync-client/`
- `Cargo.toml`: features `native` (tokio-tungstenite), `browser` (gloo-net)
- Protocol types (shared with server or copied)
- `SyncClient` struct with `sync()` method
- Native WebSocket transport using `tokio-tungstenite`
- Bridge to `ofdb-sql` embedded `SyncSession`

## Acceptance Criteria

- [ ] `cargo build --package sync-client --features native` compiles
- [ ] Connects to server from ticket 03, authenticates, syncs
- [ ] Offline edits on client converge when reconnected
- [ ] Passthrough file read streams from server

## Notes

- Depends on `ofdb-sql` with `sync` feature (embedded)
- Token provider: `Box<dyn Fn() -> String + Send + Sync>` for OAuth refresh
- Same `StoreIdentity` as server for store matching
- Reuse frame codec from `sync-test-harness`
