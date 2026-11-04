# 03: Sync Session — SQL Sync Over WebSocket

**Status:** `done`
**Type:** `task`
**Blocked by:** 02

## Description

Bridge `ofdb-sql-sync::SyncSession` to WebSocket frames. After auth succeeds, start real sync session and proxy events bidirectionally.

## What it delivers

- App sends `Hello` → `AttachRequest` → sync starts
- Offline edits on app converge with device mesh
- Offline edits on device converge with app
- Passthrough file reads stream from Full peer

## Scope

- `storage-server/src/sync.rs`:
  - `SyncSession` struct wrapping `ofdb_sql_sync::SyncSession` + WebSocket sink
  - On `AttachRequest` success: spawn `SyncSession::run()`
  - `OwnerMessage` frames → `SyncSession.handle_incoming()`
  - `SyncSession` events → `OwnerMessage`/`Ack`/`Progress` frames
  - `Ack` frames → `SyncSession.acknowledge()`

## Acceptance Criteria

- [ ] Two clients sync same store → edits converge
- [ ] Client offline, makes edits, reconnects → edits sync
- [ ] Device mesh has edits, client connects → receives edits
- [ ] Passthrough file open → streams content from Full peer
- [ ] Connection close → session cleans up

## Notes

- Use `ofdb_sql_sync::SyncSession` with `continuous` feature
- Store `DatabaseRuntime` from router state
- Reuse `sync-test-harness` frame codec (postcard + length prefix)
- No session resumption in MVP
