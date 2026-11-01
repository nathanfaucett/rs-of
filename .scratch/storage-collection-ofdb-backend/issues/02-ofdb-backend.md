# 02: Add the ofdb backend

**What to build:** Developers can pass an ofdb KV Query Handle to an adapter and use a TanStack DB collection without running a Storage service. Collection load, mutations, Refresh, and cleanup work with IndexedDB-backed storage.

**Blocked by:** 01: Make the collection backend-independent.

**Status:** ready-for-agent

- [ ] Add the adapter to the existing collection package and export it. Accept the required public KvClient methods structurally, without a runtime dependency on @of/db or a new package.
- [ ] Use get, set, delete, and scanPrefix from the current ofdb TypeScript API. Confirm compatibility with an actual KvClient rather than only a mock with a similar shape.
- [ ] Map collection paths directly to KV keys. List keys through scanPrefix using the directory plus a trailing slash; preserve collection key validation and isolation from sibling prefixes.
- [ ] Encode Uint8Array content as JSON number arrays. Decode only arrays of integer byte values from 0 through 255; reject invalid stored values. Preserve empty content and arbitrary byte content without a configurable encoding option.
- [ ] Preserve missing-read behavior and propagate query failures through the backend's acknowledgement and error contract. Do not automatically retry writes or claim that a timeout proves rollback.
- [ ] Keep database construction, sync setup, and storage ownership with the caller. Collection cleanup must not close or invalidate caller-owned Query Handles.
- [ ] Verify load, insert, update, delete, explicit Refresh, schema and identity validation, custom-byte round trips, invalid stored values, and failure handling through the public collection API where possible. Reuse existing tests and helpers rather than adding a separate framework.
- [ ] Test against a real KvDatabase opened with IndexedDB and obtain its client through client(). Verify persisted records remain available after reopening and that cleanup leaves the caller's client usable. Use a supported browser/WASM setup; the TypeScript wrapper has no in-memory constructor.
- [ ] Add a working ofdb usage example and document the byte-array storage representation, caller ownership, and required IndexedDB/WASM setup.
- [ ] Run the affected package's tests, type checks, build, and lint checks, including WebSocket regression tests. Record any validation limits.

## Notes

The parent spec's zero-copy JSON claim is incorrect: the current TypeScript DbValue API is JSON-shaped and does not accept Uint8Array. This approved plan uses one explicit byte-array encoding. No changes to ofdb Rust crates or its TypeScript wrapper are required. SQL collections, atomic multi-record batches, and committed-change subscriptions remain out of scope.
