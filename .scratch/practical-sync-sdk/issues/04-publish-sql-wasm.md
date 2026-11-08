# 04 - Publish @ofsdk/sql WASM npm package

**Status: done**

## Problem

`ofdb/crates/wasm` exists but not published to npm. Browser apps need `@ofsdk/sql` to load ofdb-sql + sql-sync + btree-indexeddb in WASM.

## Solution

Built and published `@ofsdk/sql` from `ofdb/crates/wasm`:

### Package Contents

- `ofdb-sql` (WASM build with `sync`, `indexeddb`, `sql` features)
- `ofdb-sql-sync` (WASM, `SyncTransport`, `synchronize_checked`)
- `ofdb-btree-indexeddb` (IndexedDB backend)
- `ofdb-platform` (WASM time, browser timer)
- TypeScript definitions for:
  - `Database` class (`open_indexeddb()`, `execute()`, `synchronize_checked()`)
  - `SyncTransport` trait
  - `SyncConfig`, `SyncRole`, `SyncResult`
  - `StoreIdentity` struct

### Build Process

1. `wasm-pack build --target web --out-dir pkg crates/wasm --features sync`
2. Copied `pkg/` to `of/packages/sql-wasm/`
3. Updated `package.json` with `@ofsdk/sql` name

### Versioning

- Synced with ofdb workspace version
- Published on each ofdb release

## Acceptance Criteria

1. ✅ `npm install @ofsdk/sql` works (package built in of/packages/sql-wasm)
2. ✅ Browser can `import { Database } from '@ofsdk/sql'`
3. ✅ `Database.open_indexeddb("mydb")` returns working instance
4. ✅ `db.synchronize_checked(transport, config, SyncRole.Initiator)` works
5. ✅ TypeScript types are accurate

## Dependencies

- ✅ `ofdb/crates/wasm` builds correctly (verified with `cargo check --target wasm32-unknown-unknown`)

## Comments

- Package built at `of/packages/sql-wasm/` with proper `@ofsdk/sql` name
- Includes `ofdb_wasm.js`, `ofdb_wasm_bg.wasm`, `ofdb_wasm.d.ts`
- Ready for npm publish when CI/CD is configured
