## Problem Statement

Apps (browser, native, server) need to use ofdb SQL, ofdb KV, and offs file storage with automatic sync across devices. Currently:
- ofdb/ofs engines work locally (IndexedDB, Redb)
- ofnet Iroh transport works for native mesh sync
- Storage Service has REST API but no unified "give me a handle with built-in sync transport"
- WebSocket sync endpoint exists but has hardcoded `local` endpoint lookup and no Device selection resolution
- No npm packages for ofdb-sql WASM or offs WASM
- No high-level SDK that wraps raw transport + WASM engines for apps

Apps cannot simply call `create_sql_db()` and get a database handle that syncs to another device via Management selection.

## Solution

Build a practical sync layer that connects existing working pieces:

1. **Storage Client SDK** — unified API: `create_sql_db()`, `open_sql_db()`, `sync_to_device()` returning handles with built-in transport
2. **Sync Server fix** — resolve `local` endpoint via Management selection (not hardcoded)
3. **Management Service** — expose selection API for WebSocket sync (not just Iroh)
4. **WASM npm packages** — publish `@ofsdk/sql` (ofdb-sql WASM) and `@ofsdk/fs` (offs WASM)
5. **High-level Sync SDK** — `@ofsdk/sync-sdk` wrapping raw transport + WASM engines with reactive streams

Apps sign in via IdP, call Storage Client to get database handles, Management controls which devices store what. Sync happens automatically when both devices select the resource.

## User Stories

1. As a password-manager developer, I want to call `storage.create_sql_db("vault")` and get a `SqlDatabaseHandle` that works like local ofdb-sql but syncs automatically, so I don't build sync logic.

2. As a password-manager developer, I want to call `storage.sync_to_device(vault_db, other_device_id)` and have it connect to the other device's vault, so users see their vault on all devices.

3. As an admin, I want to call `management.select_resource(device_id, vault_db_id)` to enable sync for that device, so I control where data lives.

4. As a native app developer (Tauri), I want the same `storage.create_sql_db()` API in Rust that gives me a handle with Iroh transport, so I share code with browser.

4. As a server app developer, I want to use the same SDK to sync background worker state, so all platforms use one API.

5. As a user, I want my vault to appear on my laptop after I approve my phone as a Device, so sync "just works" without manual backup/restore.

6. As a Management Service, I want WebSocket sync to respect Device selection (not just Iroh mesh), so admins control sync via the same API.

7. As a Sync Server, I want to resolve the target Device endpoint from Management selection instead of hardcoded `local`, so WebSocket sync connects to the right peer.

## Implementation Decisions

### 1. Storage Client SDK (of/crates/storage-client + of/packages/storage-client)
- Add `create_sql_db(name)`, `open_sql_db(id)`, `create_kv_store(name)`, `open_kv_store(id)`, `create_filesystem(name)`, `open_filesystem(id)`
- Each returns a handle struct containing:
  - Local engine instance (ofdb-sql WASM / ofdb-kv WASM / offs WASM)
  - Transport implementation (WebSocket for browser, Iroh for native)
  - Implements `SyncTransport` trait for `synchronize_checked()`
- Add `sync_to_device(resource_id, target_device_id)` → starts sync session via Storage Service

### 2. Sync Server Fix (of/crates/storage-server/src/sync.rs)
- Replace hardcoded `local` endpoint lookup in `authorize_sync()` with Management Service call
- On `AttachRequest`: query Management for selected target Device endpoint for this resource
- Verify both source and target Devices have selection for this resource
- Return resolved endpoint for `run_sync_session()`

### 3. Management Service Extension (of/crates/management-service)
- Add WebSocket sync selection API (parallel to existing Iroh selection)
- `select_resource_for_sync(device_id, resource_id)` / `deselect_resource_for_sync()`
- Selection state shared with Iroh mesh (single source of truth)
- Admin restrictions apply to both transports

### 4. WASM npm Packages
- `ofdb/crates/wasm` → publish as `@ofsdk/sql` (ofdb-sql + sql-sync + btree-indexeddb)
- `offs/crates/file-system-wasm` → publish as `@ofsdk/fs` (file-system WASM)
- Include TypeScript definitions for handle APIs

### 5. High-Level Sync SDK (new: of/packages/sync-sdk → `@ofsdk/sync-sdk`)
- Wraps `@ofsdk/storage-sync-client` (raw transport) + `@ofsdk/sql` / `@ofsdk/fs`
- Reactive API: `sync.changes$` (Observable/Stream), `sync.upsert()`, `sync.delete()`
- Targets: Browser (TS/RxJS), Native (Rust/tokio-stream), Server (Node/Rust)
- Handles: reconnect, catch-up via vector clock, conflict events

### 6. Transport Layer
- Browser: WebSocket to Storage Service sync endpoint
- Native: Iroh via Device Service (existing `IrohTransport` in ofdb-sql-sync)
- Server: WebSocket (same as browser) or Iroh

### 7. Handle Interface (Shared Across Platforms)
```
SqlDatabaseHandle {
  execute(sql) -> Result
  synchronize_checked(config, role) -> Result<SyncResult>
  // ofdb-sql methods work directly via transport
}
KvStoreHandle { ... }
FilesystemHandle { ... }
```

## Testing Decisions

- **Integration test**: Browser app creates SQL DB, syncs to native app via Management selection, verifies data appears on both
- **Unit test**: Sync Server resolves endpoint via Management (mock Management client)
- **Unit test**: Management selection API persists and returns correct state for WebSocket sync
- **Contract test**: WASM packages export expected TypeScript types
- **Existing patterns**: Follow `ofdb/tests/browser/src/sql_sync.rs` for WASM sync tests

## Out of Scope

- Device pairing UI (handled by @ofsdk/app)
- Admin restriction UI (handled by @ofsdk/app)
- Iroh mesh improvements (separate effort)
- Conflict resolution UI (app responsibility)
- Schema migration (ofdb-sql handles locally)
- KV/filesystem sync over WebSocket (Phase 2, SQL only for v1)

## Further Notes

- This spec connects existing working pieces (ofdb-sql, offs, ofnet, Management, Storage Service) rather than building new engines
- Phase 1: SQL only via WebSocket. KV/offs over WebSocket later.
- The "Ticket 02" work decomposes into the 5 implementation decisions above
- No new seams: Storage Client API is the single app-facing seam
