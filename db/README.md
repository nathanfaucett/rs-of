# `ofdb` — offline-first databases

`ofdb` provides two separate public facades:

- **`ofdb-sql`** (`db/sql`): relational SQL with schema catalogs, typed statements, indexes, explicit transactions, and row-level CRDT history.
- **`ofdb-kv`** (`db/kv`): typed values under UTF-8 keys with generations, tombstones, expiry, explicit transactions, and Automerge-backed history.

The public boundary is deliberately small:

- **Database** opens and controls storage, hosting, and synchronization.
- **Client** performs every query and creates explicit transactions.

Application services must use these facades, not engine- or Redb-specific crates.

## Source layout

| Area | Directories |
| --- | --- |
| Public facades | `sql`, `kv` |
| Shared values and platform | `value`, `platform` |
| SQL engine and schema | `sql-engine`, `sql-query`, `sql-schema`, `sql-translator`, `sql-history` |
| SQL persistence | `sql-engine-redb`, `sql-engine-automerge`, `sql-engine-indexeddb` |
| KV implementation | `kv-store` |
| B-tree persistence | `btree`, `btree-redb`, `btree-automerge`, `btree-indexeddb` |
| Remote query protocols | `sql-client`, `sql-server`, `sql-proto`, `sql-protocol`, `kv-client`, `kv-server`, `kv-proto` |
| Browser bindings | `wasm` |
| Test support | `sql-test`, `test-fixtures` |

The SQL and KV models are separate. They share implementation libraries where useful, not one public data interface.

## Persistence and synchronization

Native builds support in-memory and Redb persistence. Browser builds use IndexedDB through the `wasm` crate and the TypeScript wrappers in `packages/sql` and `packages/kv`.

Synchronization uses caller-supplied transport and authorization hooks. `ofdb` owns history validation, staging, atomic apply, batching, and cleanup. Callers own identity, Device/Resource authorization, peer discovery, transport, and remote delivery. Local commit notifications are not durable remote replay.

Detailed invariants and protocol limits live in [database design](design.md).

## Development

```bash
# Public facades
cargo test -p ofdb-sql --all-targets
cargo test -p ofdb-kv --all-targets

# Browser TypeScript packages
pnpm --filter @ofsdk/sql run build:all
pnpm --filter @ofsdk/kv run build:all
```

Use feature powersets and the repository validation policy for public behavior changes. WASM output is generated locally and is not a second source tree.
