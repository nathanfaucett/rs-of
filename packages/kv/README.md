# @ofsdk/kv — Key-Value Store SDK

TypeScript/JavaScript SDK for `ofdb-kv` key-value store, built from `db/kv-wasm` via `wasm-pack`.

## Installation

```bash
npm install @ofsdk/kv
# or
pnpm add @ofsdk/kv
```

## Usage

```typescript
import { createKvClient, KvValue, KvExpiry } from '@ofsdk/kv';

// Create client
const kv = await createKvClient({ memory: true });

// Set a value with expiry (milliseconds)
await kv.set("session:123", {
  value: { user: "alice", roles: ["admin"] },
  expiry: 3600000 // 1 hour
});

// Get a value
const session = await kv.get("session:123");
if (session) {
  console.log("User:", session.value.user);
}

// Delete a value
await kv.delete("session:123");

// List keys with prefix
const keys = await kv.listKeys("session:");

// Watch for changes
const unwatch = kv.watch("session:123", (value) => {
  console.log("Session changed:", value);
});
// Later: unwatch();
```

## API

### `createKvClient(options?)`

Creates a new KV client.

Options:
- `memory?: boolean` — Use in-memory storage (for testing)
- `dataDir?: string` — Persistent data directory

### `KvClient`

| Method | Description |
|--------|-------------|
| `set(key: string, value: KvValue)` | Set a value with optional expiry |
| `get(key: string)` | Get a value (returns `null` if not found/expired) |
| `delete(key: string)` | Delete a value |
| `listKeys(prefix: string)` | List keys matching prefix |
| `watch(key: string, handler)` | Watch a key for changes |
| `sync(peerId, options?)` | Sync with another device |

### Types

```typescript
interface KvValue {
  value: unknown;        // JSON-serializable value
  expiry?: number;       // Milliseconds from now (optional)
  generation?: number;   // UUIDv7 generation (internal)
}

interface KvExpiry {
  ms: number;            // Milliseconds from now
}
```

## Features

- **Automerge history** — Full conflict-free replication
- **UUIDv7 keys** — Time-ordered, unique key generations
- **Millisecond expiry** — Automatic cleanup of expired values
- **Offline-first** — All operations work locally, sync when online
- **WASM** — Runs in browser via `@ofsdk/kv`

## Building from Source

```bash
# Requires Rust toolchain (pinned in rust-toolchain.toml)
cd db/kv-wasm
wasm-pack build --target web --out-dir ../../packages/kv/pkg
```

## Related

- [db/kv/README.md](../../db/kv/README.md) — Rust crate documentation
- [db/README.md](../../db/README.md) — ofdb overview
- [packages/README.md](../README.md) — All TypeScript SDK packages
