# 05: sync-client Browser (WASM) — Connect + Auth + Sync

**Status:** `ready-for-agent`
**Type:** `task`
**Blocked by:** 04

## Description

Add `browser` feature to `sync-client` crate for WASM. Same API, uses `gloo-net` WebSocket.

## What it delivers

- `SyncClient` works in browser (WASM)
- Connects to `wss://device.example.com/storage/sync/ws`
- Syncs `ofdb-sql` (WASM build with IndexedDB backend)

## Scope

- `sync-client/Cargo.toml`: `browser` feature with `gloo-net`, `wasm-bindgen`, `wasm-bindgen-futures`, `web-sys`
- `transport::browser` module using `gloo_net::websocket`
- Same `SyncClient` API, conditional compilation
- `ofdb-sql` WASM build (IndexedDB engine)

## Acceptance Criteria

- [ ] `cargo build --package sync-client --features browser --target wasm32-unknown-unknown` compiles
- [ ] Browser app imports `sync-client`, connects, syncs
- [ ] Offline edits in browser converge when online

## Notes

- `ofdb-sql` WASM support may need `indexeddb` feature
- Same protocol, same frame codec
- Token provider works with browser OAuth flow
