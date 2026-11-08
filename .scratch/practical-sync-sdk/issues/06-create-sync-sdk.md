# 06 - Create @ofsdk/sync-sdk high-level SDK

**Status: needs-triage**

## Problem

Apps need a high-level reactive SDK that wraps raw transport (`@ofsdk/storage-sync-client`) + WASM engines (`@ofsdk/sql`, `@ofsdk/fs`). Current raw transport only gives `send/receive` bytes.

## Solution

New package: `of/packages/sync-sdk` → `@ofsdk/sync-sdk`

### API (Unified Across Targets)

```typescript
// Browser (TS) - @ofsdk/sync-sdk
import { createSync, SqlSync, KvSync, FsSync } from '@ofsdk/sync-sdk';

const sync = createSync({
  apiUrl: 'https://node.example/storage',
  appId: 'password-manager',
  userId: 'user-123',
  getToken: () => storageSyncToken, // audience: app:sync
});

// SQL
const vault = await sync.sql.open('vault-db-id');
// vault.changes$ : Observable<ChangeEvent<Secret>>
// vault.upsert(secret)
// vault.delete(id)

// KV
const settings = await sync.kv.open('settings-store-id');

// Filesystem
const fs = await sync.fs.open('documents-fs-id');
```

### Targets

| Target | Package | Stream Type |
|--------|---------|-------------|
| Browser | `@ofsdk/sync-sdk` (TS) | RxJS `Observable` |
| Native | `ofsdk_sync_sdk` (Rust) | `tokio-stream` / `async-stream` |
| Server | `@ofsdk/sync-sdk` (Node) | RxJS / Node `EventEmitter` |

### Internal Architecture

```
@ofsdk/sync-sdk
├── browser.ts     # RxJS + WebSocket + @ofsdk/sql + @ofsdk/fs
├── native.rs      # tokio-stream + Iroh/WebSocket + ofdb-sql + offs
├── server.ts      # Node + WebSocket + @ofsdk/sql + @ofsdk/fs
├── core.ts        # Shared types, conflict resolution, vector clock
└── index.ts       # Unified exports
```

### Core Types

```typescript
interface SyncConfig {
  apiUrl: string;
  appId: string;
  userId: string;
  getToken: () => Promise<string>; // returns sync token
}

interface ChangeEvent<T> {
  type: 'local-upsert' | 'local-delete' | 'remote-upsert' | 'remote-delete' | 'ack' | 'conflict';
  doc?: T;
  id?: string;
  version?: number;
  vectorClock?: Record<string, number>;
  conflict?: { local: T; remote: T; base: T; resolve: (merged: T) => void };
}

interface SyncState {
  status: 'connecting' | 'synced' | 'disconnected' | 'error';
  version: number;
  vectorClock: Record<string, number>;
}
```

### Features

1. **Reactive stream**: `sync.changes$` emits all changes (local + remote)
2. **Optimistic writes**: `upsert()`/`delete()` update local immediately, sync in background
3. **Auto-reconnect**: On disconnect, reconnect + catch-up via vector clock
4. **Conflict events**: Emits `conflict` event, app calls `resolve(merged)`
5. **Transport abstraction**: Same API for WebSocket (browser/server) and Iroh (native)

## Acceptance Criteria

1. Browser app imports `@ofsdk/sync-sdk`, calls `createSync()`, gets reactive SQL sync
2. Native Rust app uses `ofsdk_sync_sdk::Sync::new()`, same API
3. Server Node app uses same package
4. All three targets sync with each other via Storage Service
5. Offline writes sync on reconnect

## Dependencies

- Ticket 01 (Storage Client API)
- Ticket 04 (`@ofsdk/sql` published)
- Ticket 05 (`@ofsdk/fs` published)

## Comments
