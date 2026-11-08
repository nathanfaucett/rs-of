# 06 - Create @ofsdk/sync-sdk high-level SDK

**Status: in-progress**

## Problem

Apps need a high-level reactive SDK that wraps raw transport (`@ofsdk/storage-sync-client`) + WASM engines (`@ofsdk/sql`, `@ofsdk/fs`). Current raw transport only gives `send/receive` bytes.

## Solution

New package: `of/packages/sync-sdk` → `@ofsdk/sync-sdk`

### API (Unified Across Targets)

```typescript
// Browser (TS) - @ofsdk/sync-sdk
import { createSync, SqlSync, KvSync, FsSync } from "@ofsdk/sync-sdk";

const sync = createSync({
  apiUrl: "https://node.example/storage",
  appId: "password-manager",
  userId: "user-123",
  getToken: () => storageSyncToken, // audience: app:sync
});

// SQL
const vault = await sync.sql.open("vault-db-id");
// vault.changes$ : Observable<ChangeEvent<Secret>>
// vault.upsert(secret)
// vault.delete(id)

// KV
const settings = await sync.kv.open("settings-store-id");

// Filesystem
const fs = await sync.fs.open("documents-fs-id");
```

### Targets

| Target  | Package                  | Stream Type                     |
| ------- | ------------------------ | ------------------------------- |
| Browser | `@ofsdk/sync-sdk` (TS)   | RxJS `Observable`               |
| Native  | `ofsdk_sync_sdk` (Rust)  | `tokio-stream` / `async-stream` |
| Server  | `@ofsdk/sync-sdk` (Node) | RxJS / Node `EventEmitter`      |

### Implementation

Created `of/packages/sync-sdk` with:

- `src/core.ts` - Shared types (SyncConfig, ChangeEvent, SyncState, SyncOptions, handle interfaces)
- `src/browser.ts` - Browser implementation using RxJS, WebSocket, @ofsdk/sql, @ofsdk/fs
- `src/node.ts` - Node.js implementation using RxJS, WebSocket
- `src/index.ts` - Main entry point with platform detection
- `tsconfig.json`, `tsconfig.browser.json`, `tsconfig.node.json`
- `package.json` with proper exports

### Features

1. **Reactive stream**: `sync.changes$` emits all changes (local + remote)
2. **Optimistic writes**: `upsert()`/`delete()` update local immediately, sync in background
3. **Auto-reconnect**: On disconnect, reconnect + catch-up via vector clock
4. **Conflict events**: Emits `conflict` event, app calls `resolve(merged)`
5. **Transport abstraction**: Same API for WebSocket (browser/server) and Iroh (native)

### Acceptance Criteria

1. ⏳ Browser app imports `@ofsdk/sync-sdk`, calls `createSync()`, gets reactive SQL sync
2. ⏳ Native Rust app uses `ofsdk_sync_sdk::Sync::new()`, same API
3. ⏳ Server Node app uses same package
4. ⏳ All three targets sync with each other via Storage Service
5. ⏳ Offline writes sync on reconnect

## Dependencies

- ✅ Ticket 01 (Storage Client API)
- ✅ Ticket 04 (`@ofsdk/sql` published)
- ✅ Ticket 05 (`@ofsdk/fs` published)

## Comments

- Browser implementation uses `@ofsdk/storage-client` for REST + `@ofsdk/storage-sync-client` for WebSocket transport
- Native implementation will use `of-storage-client` Rust crate with Iroh transport
- Vector clock for catch-up reuses ofdb-sql-sync concepts
- Conflict resolution is app responsibility (manual strategy)
