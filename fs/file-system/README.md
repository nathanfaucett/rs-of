# file-system — Offline-First Filesystem

`file-system` stores data locally so reads and writes continue without network access.

`file-system` owns filesystem paths, metadata, content, persistence, and synchronization.

- **Full devices** — Retain metadata and content.
- **Passthrough devices** — Retain metadata only; read content from an available selected Full peer; cannot write.
- **Resource deletion** — A tombstone; device deselection removes only that device's local copy.

## Features

- **Offline-first** — All operations work without network
- **Content-addressed storage** — Deduplicated blobs, efficient sync
- **CRDT synchronization** — Conflict-free replication across devices
- **Selection policies** — Per-device, per-resource sync control
- **Passthrough mode** — Lightweight devices that don't store content
- **WASM support** — Browser build via `file-system-wasm`

## Quick Start

```rust
use file_system::{FileSystem, FileSystemConfig};

// Open or create a filesystem
let fs = FileSystem::open(FileSystemConfig {
    path: "/data/my-fs".into(),
    ..Default::default()
}).await?;

// Write a file
fs.write("/hello.txt", b"Hello, world!").await?;

// Read a file
let content = fs.read("/hello.txt").await?;

// List files
let entries = fs.list("/").await?;

// Create directory
fs.create_dir("/photos").await?;
```

## Sync

```rust
use file_system::{FileSystem, SyncConfig};
use iroh::Endpoint;

let fs = FileSystem::open(config).await?;

// Configure sync with a peer
let sync_config = SyncConfig {
    peer: endpoint.node_addr(),
    // ... selection policies from device-service ...
};

// Start sync session
let session = fs.sync(sync_config).await?;

// Monitor progress
while let Some(event) = session.next().await {
    println!("Sync event: {:?}", event);
}
```

## Selection Policies

Sync is governed by `device-service` selection policies:

- **Selected resources** — Which databases/filesystems a device syncs
- **Admin allowance** — Whether admin can override device selection
- **Device state** — Approved, pending, revoked

A device only syncs resources it has explicitly selected (or admin has allowed).

## WASM

Build for browser:

```bash
cd fs/file-system-wasm
wasm-pack build --target web
```

The generated `pkg/` is checked into `packages/file-system/` for npm publishing as `@ofsdk/file-system`.

## Architecture

```
file-system
  ├── paths (B-tree, UUIDv7 keys)
  ├── metadata (size, mtime, permissions)
  ├── content (chunked, deduplicated blobs)
  ├── persistence (Redb, IndexedDB)
  ├── sync (CRDT, tombstones, selection policies)
  └── transport (Iroh streams via iroh-chain)
```

## Testing

```bash
# Native
cargo test -p file-system --all-targets

# WASM
cd fs/file-system-wasm
wasm-pack test --headless --firefox
```

## Related

- [fs/README.md](../README.md) — offs overview
- [db/README.md](../../db/README.md) — Offline-first databases
- [net/README.md](../../net/README.md) — Mesh networking
- [packages/README.md](../../packages/README.md) — TypeScript SDKs
