# 05 - Publish @ofsdk/fs WASM npm package

**Status: needs-triage**

## Problem

`offs/crates/file-system-wasm` exists but not published to npm. Browser apps need `@ofsdk/fs` for offs file system in WASM.

## Solution

Publish `@ofsdk/fs` from `offs/crates/file-system-wasm`:

### Package Contents

- `file-system` (WASM build)
- TypeScript definitions for:
  - `FileSystem` class (`open()`, `create()`, `read()`, `write()`, `metadata_sync()`)
  - `SyncTransport` implementation for filesystem sync
  - `ResidencyMode` (Full, Passthrough)
  - `FileEntry`, `FileMetadata`

### Build Process

1. Add `package.json` to `offs/crates/file-system-wasm`:
   ```json
   {
     "name": "@ofsdk/fs",
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

- Sync with offs workspace version
- Publish on each offs release

## Acceptance Criteria

1. `npm install @ofsdk/fs` works
2. Browser can `import { FileSystem } from '@ofsdk/fs'`
3. `FileSystem.open(...)` returns working instance
4. Filesystem sync works via `SyncTransport`
5. TypeScript types are accurate

## Dependencies

- `offs/crates/file-system-wasm` builds correctly (verify)
- CI/CD for npm publish

## Comments
