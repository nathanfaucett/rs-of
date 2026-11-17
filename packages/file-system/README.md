# @ofsdk/file-system — Filesystem SDK

TypeScript/JavaScript SDK for `file-system` offline-first filesystem, built from `fs/file-system-wasm` via `wasm-pack`.

## Installation

```bash
npm install @ofsdk/file-system
# or
pnpm add @ofsdk/file-system
```

## Usage

```typescript
import { createFileSystemClient, FileEntry } from '@ofsdk/file-system';

// Create client
const fs = await createFileSystemClient({ path: "/data/my-fs" });

// Write a file
await fs.write("/hello.txt", new TextEncoder().encode("Hello, world!"));

// Read a file
const content = await fs.read("/hello.txt");
console.log(new TextDecoder().decode(content));

// List directory
const entries: FileEntry[] = await fs.list("/photos");
for (const entry of entries) {
  console.log(`${entry.isDir ? 'dir' : 'file'} ${entry.name} (${entry.size} bytes)`);
}

// Create directory
await fs.createDir("/documents");

// Delete file
await fs.delete("/old.txt");

// Get file info
const info = await fs.stat("/hello.txt");
console.log("Size:", info.size, "Modified:", info.mtime);
```

## Sync

```typescript
import { createFileSystemClient, SyncConfig } from '@ofsdk/file-system';
import { createNetClient } from '@ofsdk/net';

const fs = await createFileSystemClient({ path: "/data/my-fs" });
const net = await createNetClient();

// Connect to peer
await net.connect(trustedPeerNodeAddr);

// Configure sync
const syncConfig: SyncConfig = {
  peer: trustedPeerNodeAddr,
  // Selection policies from device-service
  selectedResources: [
    { kind: "filesystem", id: "my-fs", selected: true }
  ]
};

// Start sync
const session = await fs.sync(syncConfig);

// Monitor progress
for await (const event of session) {
  console.log("Sync:", event.type, event.path);
}
```

## API

### `createFileSystemClient(options)`

Creates a new filesystem client.

Options:
- `path: string` — Filesystem root path
- `memory?: boolean` — Use in-memory storage (for testing)

### `FileSystemClient`

| Method | Description |
|--------|-------------|
| `write(path: string, data: Uint8Array)` | Write file (creates parents) |
| `read(path: string)` | Read file content |
| `list(path: string)` | List directory entries |
| `createDir(path: string)` | Create directory |
| `delete(path: string)` | Delete file or empty directory |
| `stat(path: string)` | Get file metadata |
| `sync(config: SyncConfig)` | Start sync session |

### Types

```typescript
interface FileEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  mtime: number;  // Unix timestamp (ms)
}

interface FileInfo {
  size: number;
  mtime: number;
  isDir: boolean;
}

interface SyncConfig {
  peer: string;                    // Peer node address
  selectedResources: ResourceSelection[];
}

interface ResourceSelection {
  kind: "database" | "filesystem";
  id: string;
  selected: boolean;
}

interface SyncEvent {
  type: "start" | "progress" | "complete" | "error";
  path?: string;
  bytesTransferred?: number;
  error?: string;
}
```

## Features

- **Offline-first** — All operations work without network
- **Content-addressed** — Deduplicated blob storage
- **CRDT sync** — Conflict-free replication across devices
- **Selection policies** — Per-device sync control via device-service
- **Passthrough mode** — Lightweight devices (metadata only)
- **WASM** — Runs in browser via `@ofsdk/file-system`

## Building from Source

```bash
# Requires Rust toolchain (pinned in rust-toolchain.toml)
cd fs/file-system-wasm
wasm-pack build --target web --out-dir ../../packages/file-system/pkg
```

The `pkg/` directory is checked in for npm publishing.

## Related

- [fs/file-system/README.md](../../fs/file-system/README.md) — Rust crate documentation
- [fs/README.md](../../fs/README.md) — offs overview
- [packages/README.md](../README.md) — All TypeScript SDK packages
