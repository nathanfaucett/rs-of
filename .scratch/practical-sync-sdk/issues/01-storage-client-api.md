# 01 - Storage Client: add create_sql_db / open_sql_db / sync_to_device

**Status: needs-triage**

## Problem

Apps cannot get a database handle with built-in sync transport. Current `@ofsdk/storage-client` only has REST API for database CRUD and filesystem WebSocket. No SQL/KV/offs handle that implements `SyncTransport` for `synchronize_checked()`.

## Solution

Extend Storage Client (Rust crate + npm package) with:

```rust
// of/crates/storage-client/src/lib.rs
pub struct StorageClient { ... }

impl StorageClient {
    // SQL
    pub async fn create_sql_db(&self, name: &str) -> Result<SqlDatabaseHandle>;
    pub async fn open_sql_db(&self, id: DatabaseId) -> Result<SqlDatabaseHandle>;
    pub async fn list_sql_dbs(&self) -> Result<Vec<DatabaseInfo>>;

    // KV
    pub async fn create_kv_store(&self, name: &str) -> Result<KvStoreHandle>;
    pub async fn open_kv_store(&self, id: KvStoreId) -> Result<KvStoreHandle>;
    pub async fn list_kv_stores(&self) -> Result<Vec<KvStoreInfo>>;

    // Filesystem
    pub async fn create_filesystem(&self, name: &str) -> Result<FilesystemHandle>;
    pub async fn open_filesystem(&self, id: FilesystemId) -> Result<FilesystemHandle>;
    pub async fn list_filesystems(&self) -> Result<Vec<FilesystemInfo>>;

    // Sync
    pub async fn sync_to_device(&self, resource_id: ResourceId, target_device: DeviceId) -> Result<SyncSession>;
}
```

### Handle Requirements

Each handle (`SqlDatabaseHandle`, `KvStoreHandle`, `FilesystemHandle`) must:
- Contain local engine instance (ofdb-sql WASM / ofdb-kv WASM / offs WASM)
- Contain transport implementation (WebSocket for browser, Iroh for native)
- Implement `ofdb_sql::SyncTransport` (for SQL) / equivalent for KV/offs
- Expose engine methods directly (`execute()`, `synchronize_checked()`, etc.)

### Transport Selection

- Browser: WebSocket to Storage Service sync endpoint (`wss://<base>/sync/ws`)
- Native: Iroh via Device Service (reuse `IrohTransport` from ofdb-sql-sync)
- Server: WebSocket (same as browser)

## Implementation Notes

- Browser: Load `@ofsdk/sql` WASM, create `BrowserSyncTransport` from `@ofsdk/storage-sync-client`
- Native: Link `ofdb-sql` natively, use `IrohTransport`
- Storage Service must support `sync_to_device` endpoint (POST /sync/connect)

## Dependencies

- `ofdb/crates/wasm` must publish `@ofsdk/sql` (Ticket 04)
- `offs/crates/file-system-wasm` must publish `@ofsdk/fs` (Ticket 05)
- `of/crates/storage-sync-client` provides `BrowserSyncTransport` (exists)

## Acceptance Criteria

1. Browser app can call `storage.create_sql_db("vault")` → gets `SqlDatabaseHandle`
2. Handle implements `SyncTransport` → `db.synchronize_checked()` works
3. `storage.sync_to_device(vault_id, device_id)` starts sync session
4. Same API works in Rust native (different transport)

## Comments
