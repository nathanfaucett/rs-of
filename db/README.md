# ofdb — Offline-First Databases

`ofdb` provides two self-contained database models for local-first applications:

- **`ofdb-sql`** — Relational SQL engine with schema catalogs, typed statements, secondary indexes, and row-level CRDT synchronization.
- **`ofdb-kv`** — Key-value store for shared typed values with Automerge-backed history, UUIDv7 key generations, and millisecond expiry.

Both facades separate database management from query execution:

- **`Database` (Control Handle)** — Manages storage lifecycle (`open`, `in_memory`), hosts remote endpoints, and runs sync sessions. Does not execute queries.
- **`Client` (Query Handle)** — The sole interface for reading and writing. Get an embedded client via `database.client()` or connect remotely. Same API for local memory, on-disk Redb, or network.
- **`Transaction`** — Explicit multi-statement transactions with atomic `commit()` and `rollback()`. Remote transactions stream over one connection and roll back if the client handle drops.

## Crates

| Crate | Purpose |
|-------|---------|
| `sql` | SQL facade (`ofdb-sql`) — public API, macros, query builder |
| `kv` | KV facade (`ofdb-kv`) — public API, macros |
| `sync` | Sync protocol, Iroh transport, staging, batching, cleanup |
| `btree` | B-tree core (keys, values, changes, transactions) |
| `btree-redb` | Redb-backed B-tree storage |
| `btree-automerge` | Automerge-backed B-tree for CRDT sync |
| `btree-indexeddb` | IndexedDB-backed B-tree (WASM) |
| `sql-engine` | SQL execution engine (planner, executor) |
| `sql-engine-redb` | Redb-backed SQL storage |
| `sql-engine-automerge` | Automerge-backed SQL history |
| `sql-engine-indexeddb` | IndexedDB-backed SQL storage (WASM) |
| `sql-query` | SQL query AST, translator, result types |
| `sql-schema` | Schema catalog, migrations, column types |
| `sql-protocol` | SQL wire protocol (sync frames, CRDT ops) |
| `sql-history` | Validated history for sync, conflict resolution |
| `sql-server` | Remote SQL server (Unix/TCP, TLS) |
| `sql-client` | Remote SQL client (Unix/TCP, TLS) |
| `kv-store` | KV storage backend (Redb, Automerge, IndexedDB) |
| `kv-server` / `kv-client` | Remote KV server/client |
| `platform` | Execution platform (async, time, sync) |
| `value` | Shared `Value` type (UUID, JSON, blobs, etc.) |

## Features

- **`native-stage`** — Enable `ofdb-sql` native staging for CRDT sync (used by `device-service`)
- **`remote`** — Remote client/server transport (Unix sockets, TLS)
- **`redb`** — On-disk Redb persistence
- **`in-memory`** — In-memory storage for tests
- **`validated-history`** — Sync validation, `SyncTransport`, `SyncResult` exports
- **`wasm`** — Browser builds (IndexedDB, wasm-bindgen)

## Quick Start

```rust
use ofdb_sql::{Database, Client};

// In-memory database
let db = Database::in_memory();
let client = db.client();

// Execute SQL
client.execute_sql("CREATE TABLE users (id UUID PRIMARY KEY, name TEXT)", None).await?;
client.execute_sql("INSERT INTO users (id, name) VALUES (uuid_v7(), 'Alice')", None).await?;

// Query
let rows = client.query("SELECT * FROM users", None).await?;
```

## Sync

```rust
use ofdb_sql::Database;
use ofdb_sync::{SyncTransport, SyncRole, SessionConfig};

// Peer A (initiator)
let db_a = Database::in_memory();
// ... connect via Iroh ...
let transport = IrohSyncTransport::new(connection);
let result = db_a.exchange_history(&mut transport, &SessionConfig::default(), SyncRole::Initiator, staging_dir, &["users"], validator, authorizer, reauthorize).await?;

// Peer B (responder)
let db_b = Database::in_memory();
let transport = IrohSyncTransport::new(connection);
let result = db_b.exchange_history(&mut transport, &SessionConfig::default(), SyncRole::Responder, staging_dir, &["users"], validator, authorizer, reauthorize).await?;
```

## Feature Flags for `ofdb-sql`

| Feature | Description |
|---------|-------------|
| `sql` | Core SQL API (macros, translator) |
| `redb` | Redb storage backend |
| `in-memory` | In-memory storage |
| `remote` | Unix/TCP remote transport |
| `native-stage` | CRDT staging for sync |
| `validated-history` | Sync validation types |
| `server` | Remote SQL server |
| `sync` | Sync protocol (includes `native-stage`) |

## WASM

The `db/wasm` crate is a standalone `wasm-pack` workspace (own `Cargo.lock`). It is **excluded** from the Cargo workspace. Build with:

```bash
cd db/wasm
wasm-pack build --target web
```

The generated `pkg/` is checked into `packages/sql-wasm/` and `packages/kv/` for npm publishing.

## Testing

```bash
# All ofdb crates (from workspace root)
cargo test --workspace -p ofdb-sql -p ofdb-kv -p ofdb-sync ...

# With all feature combinations
cargo hack test --feature-powerset -p ofdb-sql --all-targets
```

## Architecture

```
ofdb-sql (facade)
  ├── sql-engine (execution)
  │     ├── sql-engine-redb
  │     ├── sql-engine-automerge
  │     └── sql-engine-indexeddb
  ├── sql-query (AST, translator)
  ├── sql-schema (catalog, migrations)
  ├── sql-protocol (wire format)
  ├── sql-history (validated history, CRDT)
  └── sql-client / sql-server (remote)

ofdb-kv (facade)
  ├── kv-store (Redb, Automerge, IndexedDB)
  └── kv-server / kv-client (remote)

sync (protocol)
  ├── staging, batching, cleanup
  └── iroh-transport
```

## Related

- [fs/README.md](../fs/README.md) — Offline-first filesystem (`file-system`)
- [net/README.md](../net/README.md) — Mesh networking (`iroh-chain`)
- [packages/README.md](../packages/README.md) — TypeScript SDK packages
