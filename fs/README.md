# `offs` — offline-first filesystem

`offs` owns filesystem paths, metadata, content, persistence, history, and reconciliation. It does not own Installation identity, Device authorization, or generic network routing.

## Device-local residency

- **Full** retains metadata and content.
- **Passthrough** retains metadata and reads content from an authorized available Full provider. It cannot write.
- **Resource deletion** creates a synchronized tombstone.
- **Deselection** removes only the selected Device's local replica; it does not delete the logical Resource.

Device approval and Resource Selection belong to the `of` Device and Storage contexts. See [target architecture](../docs/architecture.md).

## Crates

| Crate | Purpose |
| --- | --- |
| `file-system` | Core filesystem and synchronization contracts |
| `file-system-wasm` | Browser WebAssembly bindings |

The TypeScript wrapper is `packages/file-system`. It is a private workspace package, not a published public SDK.

## Development

```bash
cargo test -p file-system --all-targets
pnpm --filter @ofsdk/file-system run build
```

The WASM crate has its own lockfile and generated output. Build everything from this checkout; no sibling `offs` repository is required.

Filesystem synchronization is not qualified by SQL tests or mesh connectivity alone. Full/Passthrough behavior needs real application and multi-Device acceptance before it is presented as complete.
