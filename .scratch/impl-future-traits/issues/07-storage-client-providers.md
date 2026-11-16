# 07: storage client/sdk + tauri fetch — token and handler providers generic

**What to build:** The token provider used by the storage client (and its subject-token wrapper), the browser token provider in the storage SDK, and the Tauri fetch handler stop being `Arc<dyn Fn(...) -> Pin<Box<dyn Future>>>` / `Box<dyn Fn(...) -> ...>`. `StorageClient`, its store handles, `StorageSdk`, and the Tauri backend become generic over the provider closure/future pair; constructor sites and tests updated.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] storage-client, storage-sdk, and the Tauri fetch plugin compile with no boxed/dyn async provider
- [ ] Storage-client sync tests pass; clippy/rustfmt clean
