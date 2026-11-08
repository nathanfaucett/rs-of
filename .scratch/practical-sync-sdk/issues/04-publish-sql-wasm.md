# 04 - Publish @ofsdk/sql WASM npm package

**Status: needs-triage**

## Problem

`ofdb/crates/wasm` exists but not published to npm. Browser apps need `@ofsdk/sql` to load ofdb-sql + sql-sync + btree-indexeddb in WASM.

## Solution

Publish `@ofsdk/sql` from `ofdb/crates/wasm`:

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

1. Add `package.json` to `ofdb/crates/wasm`:
   ```json
   {
     "name": "@ofsdk/sql",
     "version": "0.1.0",
     "main": "index.js",
     "types": "index.d.ts",
     "files": ["*.js", "*.wasm", "*.d.ts"]
   }
   ```
2. `wasm-pack build --target web --out-dir pkg`
3. Copy `pkg/` to npm package root
4. Publish to npm registry

### Versioning

- Sync with ofdb workspace version
- Publish on each ofdb release

## Acceptance Criteria

1. `npm install @ofsdk/sql` works
2. Browser can `import { Database } from '@ofsdk/sql'`
3. `Database.open_indexeddb("mydb")` returns working instance
4. `db.synchronize_checked(transport, config, SyncRole.Initiator)` works
5. TypeScript types are accurate

## Dependencies

- `ofdb/crates/wasm` builds correctly (verify)
- CI/CD for npm publish

## Comments
