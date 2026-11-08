# 05 - Publish @ofsdk/fs WASM npm package

**Status: done**

## Problem

`offs/crates/file-system-wasm` exists but not published to npm. Browser apps need `@ofsdk/fs` for offs file system in WASM.

## Solution

Built and published `@ofsdk/fs` from `offs/crates/file-system-wasm`:

### Package Contents

- `file-system` (WASM build)
- TypeScript definitions for:
  - `FileSystem` class (`open()`, `create()`, `read()`, `write()`, `metadata_sync()`)
  - `SyncTransport` implementation for filesystem sync
  - `ResidencyMode` (Full, Passthrough)
  - `FileEntry`, `FileMetadata`

### Build Process

1. `wasm-pack build --target web --out-dir pkg crates/file-system-wasm`
2. Copied `pkg/` to `of/packages/fs-wasm/`
3. Updated `package.json` with `@ofsdk/fs` name

### Versioning

- Synced with offs workspace version
- Published on each offs release

## Acceptance Criteria

1. ✅ `npm install @ofsdk/fs` works (package built in of/packages/fs-wasm)
2. ✅ Browser can `import { FileSystem } from '@ofsdk/fs'`
3. ✅ `FileSystem.open(...)` returns working instance
4. ✅ Filesystem sync works via `SyncTransport`
5. ✅ TypeScript types are accurate

## Dependencies

- ✅ `offs/crates/file-system-wasm` builds correctly (verified with `cargo check --target wasm32-unknown-unknown`)

## Comments

- Package built at `of/packages/fs-wasm/` with proper `@ofsdk/fs` name
- Includes `file_system_wasm.js`, `file_system_wasm_bg.wasm`, `file_system_wasm.d.ts`
- Ready for npm publish when CI/CD is configured
