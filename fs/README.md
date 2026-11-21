# `offs` — offline-first filesystem

`offs` owns filesystem paths, metadata, content, persistence, history, and reconciliation. It does not own Installation identity, Device authorization, or generic network routing.

## Device-local residency

- **Full** retains metadata and content.
- **Passthrough** retains metadata and reads content from an authorized available Full provider. It cannot write.
- **Resource deletion** creates a synchronized tombstone.
- **Deselection** removes only the selected Device's local replica; it does not delete the logical Resource.

Device approval and Resource Selection belong to the `of` Device and Storage contexts. See [target architecture](../docs/architecture.md).

## Crates

| Crate              | Purpose                                           |
| ------------------ | ------------------------------------------------- |
| `file-system`      | Core filesystem and synchronization contracts     |
| `file-system-fuse` | Linux FUSE mounts for Full-residency File Systems |
| `file-system-wasm` | Browser WebAssembly bindings                      |

The TypeScript wrapper is `packages/file-system`. It is a private workspace package, not a published public SDK.

## Development

```bash
cargo test -p file-system --all-targets
pnpm --filter @ofsdk/file-system run build
```

The WASM crate has its own lockfile and generated output. Build everything from this checkout; no sibling `offs` repository is required.

Filesystem synchronization is not qualified by SQL tests or mesh connectivity alone. Full/Passthrough behavior needs real application and multi-Device acceptance before it is presented as complete.

## Linux FUSE mounts

`file-system-fuse` mounts an already opened File System only when the root and every entry are Full-resident and all current content is locally retained. It supports lookup, attributes, offset reads, ordinary and exclusive file creation, revision-checked offset and append writes, open-time and size-attribute truncation, flush/fsync/close, directory creation, file unlink, empty-directory removal, no-replace file and directory-tree rename, and directory iteration with explicit unmount. Each admitted write, truncation, or file rename completes the existing durable Content Revision boundary before reporting success. Extension zero-fills new bytes. Other attribute mutations, atomic rename replacement, recursive deletion, Passthrough retrieval, non-Linux platforms, and complete concurrent-writer POSIX behavior are unsupported.

A real Linux scenario with two native Storage Full Replicas, authorized Iroh transport, explicit operation policy, authorized content fetch, offline edits, Storage reopen, and protocol-version-1 convergence is qualified. Remote metadata still requires explicit content materialization before mounted reads. This scenario does not establish blanket multi-Device, Passthrough, or other-platform support. See the [adapter overview](file-system-fuse/README.md) for mount prerequisites, test commands, and exact limits.
