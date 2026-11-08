# 01 - Storage Client: add create_sql_db / open_sql_db / sync_to_device

**Status: in-progress**

## Problem

Apps cannot get a database handle with built-in sync transport. Current `@ofsdk/storage-client` only has REST API for database CRUD and filesystem WebSocket. No SQL/KV/offs handle that implements `SyncTransport` for `synchronize_checked()`.

## Solution

Extended Storage Client (Rust crate + npm package) with:

### Rust crate (of/crates/storage-client)

- Added `SqlDatabaseHandle`, `KvStoreHandle`, `FilesystemHandle` with built-in transport
- Added `StorageClient::create_sql_db()`, `open_sql_db()`, `list_sql_dbs()`
- Added `StorageClient::create_kv_store()`, `open_kv_store()`, `list_kv_stores()`
- Added `StorageClient::create_filesystem()`, `open_filesystem()`, `list_filesystems()`
- Added `StorageClient::sync_to_device()`
- Native (Rust) and browser (WASM) implementations

### TypeScript package (of/packages/storage-client)

- Added `SqlDatabaseHandle`, `KvStoreHandle`, `FilesystemHandle` classes
- Added `createSqlDatabase()`, `openSqlDatabase()`, `listSqlDatabases()`
- Added `createKvStore()`, `openKvStore()`, `listKvStores()`
- Added `createFilesystem()`, `openFilesystem()`, `listFileSystems()`
- Added `syncToDevice()`

### Transport

- Browser: WebSocket via `@ofsdk/storage-sync-client` + `@ofsdk/sql` WASM
- Native: Iroh via Device Service (reuses `IrohTransport` from ofdb-sql-sync)
- Server: WebSocket (same as browser)

## Implementation Notes

- Browser: Load `@ofsdk/sql` WASM, create `BrowserSyncTransport` from `@ofsdk/storage-sync-client`
- Native: Link `ofdb-sql` natively, use `IrohTransport`
- Storage Service must support `sync_to_device` endpoint (POST /sync/connect)

## Acceptance Criteria

1. ✅ Browser app can call `storage.create_sql_db("vault")` → gets `SqlDatabaseHandle`
2. ⏳ Handle implements `SyncTransport` → `db.synchronize_checked()` works
3. ✅ `storage.sync_to_device(vault_id, device_id)` starts sync session
4. ✅ Same API works in Rust native (different transport)

## Dependencies

- ✅ `@ofsdk/sql` published (Ticket 04)
- ✅ `@ofsdk/fs` published (Ticket 05)
- ✅ `@ofsdk/storage-sync-client` provides `BrowserSyncTransport` (exists)

## Comments

- KV/offs sync over WebSocket deferred to Phase 2 (SQL only for v1)
- WASM packages built and published to of/packages/sql-wasm and of/packages/fs-wasm
