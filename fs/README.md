# offs — Offline-First Filesystem

`file-system` stores data locally so reads and writes continue without network access.

`file-system` owns filesystem paths, metadata, content, persistence, and synchronization.

- **Full devices** — Retain metadata and content.
- **Passthrough devices** — Retain metadata only; read content from an available selected Full peer; cannot write.
- **Resource deletion** — A tombstone; device deselection removes only that device's local copy.

## Crates

| Crate | Purpose |
|-------|---------|
| `file-system` | Core filesystem — paths, metadata, content, persistence, sync |
| `file-system-wasm` | WASM build for browser (wasm-pack, own Cargo.lock) |

## Features

| Feature | Description |
|---------|-------------|
| `std` | Standard library (default) |
| `sync` | Enable synchronization protocol |

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
```

## Sync

```rust
use file_system::{FileSystem, SyncConfig};
use iroh::Endpoint;

let fs = FileSystem::open(config).await?;

// Configure sync with a peer
let sync_config = SyncConfig {
    peer: endpoint.node_addr(),
    // ... selection policies ...
};

// Start sync session
let session = fs.sync(sync_config).await?;

// Monitor progress
while let Some(event) = session.next().await {
    println!("Sync event: {:?}", event);
}
```

## WASM

The `file-system-wasm` crate is a standalone `wasm-pack` workspace (own `Cargo.lock`). It is **excluded** from the Cargo workspace. Build with:

```bash
cd fs/file-system-wasm
wasm-pack build --target web
```

The generated `pkg/` is checked into `packages/file-system/` for npm publishing.

## Testing

```bash
# Core filesystem
cargo test -p file-system --all-targets

# WASM (from fs/file-system-wasm)
wasm-pack test --headless --firefox
```

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

## Selection Policies

Sync is governed by `device-service` selection policies:

- **Selected resources** — Which databases/filesystems a device syncs
- **Admin allowance** — Whether admin can override device selection
- **Device state** — Approved, pending, revoked

## Related

- [db/README.md](../db/README.md) — Offline-first databases (`ofdb-sql`, `ofdb-kv`)
- [net/README.md](../net/README.md) — Mesh networking (`iroh-chain`)
- [packages/README.md](../packages/README.md) — TypeScript SDK packages
