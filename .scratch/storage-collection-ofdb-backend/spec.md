# Spec: Generic Storage Backend for storage-db-collection

Status: ready-for-agent

## Problem Statement

`@aicacia/storage-db-collection` only works with the WebSocket-based `@aicacia/storage-client`. It cannot be used with `ofdb`'s embedded KV store (`@of/db`), which is the persistence layer the `of` workspace wants to adopt for offline-first local storage. Developers who want a TanStack DB collection backed by ofdb KV have no way to wire it up without the storage-service WebSocket protocol.

## Solution

Decouple `storage-db-collection` from the WebSocket `StorageConnection` by introducing a minimal generic `StorageBackend` interface. The existing WebSocket client becomes one adapter; a new ofdb KV adapter implements the same interface over `KvClient`, enabling the collection to run directly against ofdb's IndexedDB-backed KV store.

## User Stories

1. As a developer using `@of/db`, I want to create a TanStack DB collection backed by ofdb KV, so that I get local-first reactive state without running a storage service.
2. As a developer using `@aicacia/storage-client`, I want the existing WebSocket-backed collection to keep working unchanged, so that I don't have to migrate existing code.
3. As a developer, I want the collection's snapshot/load/sync logic to be backend-agnostic, so that both WebSocket storage and ofdb KV share the same correctness guarantees (schema validation, key identity checks, unacknowledged-write detection).
4. As a developer, I want to list all records in a collection directory from ofdb KV, so that initial snapshot loading works without a filesystem listing API.
5. As a developer, I want the collection to handle backends with no connection lifecycle (embedded stores), so that disposal and reconnection logic doesn't break on ofdb.
6. As a developer, I want serialize/deserialize hooks to bridge between byte content and ofdb's JSON values, so that non-JSON record formats still work.
7. As a maintainer, I want the generic backend interface to be minimal (read, write, delete, list, optional close), so that future backends are easy to add.

## Implementation Decisions

- Introduce a `StorageBackend` interface in `storage-db-collection` with methods: `read(key) → Uint8Array | undefined`, `write(key, bytes) → void`, `delete(key) → void`, `list(prefix) → string[]`, and optional `close()`. This replaces the `client.openSocket()` option.
- The `StorageCollectionOptions.client` field changes from `{ openSocket(...) }` to a `connect(signal?) → Promise<StorageBackend>` factory, preserving the existing reconnect-on-failure pattern.
- Socket lifecycle events (`close`/`error` on the connection) become optional on the backend. Embedded backends like ofdb omit them; the collection treats absence as "never disconnects."
- Path-to-key mapping is identity: the existing `directory/key.ext` path strings are used directly as KV keys. No transformation needed.
- Directory listing maps to prefix scan: `list(directory + "/")` is implemented via ofdb's `scanPrefix`.
- An ofdb adapter module wraps `KvClient`: `read` → `get` + decode, `write` → `set` with JSON-encoded bytes, `delete` → `delete`, `list` → `scanPrefix`. Bytes are encoded as base64 or JSON array within ofdb's `DbValue`.
- The existing WebSocket path is preserved as an adapter that wraps `StorageConnection` into the `StorageBackend` interface, keeping `StorageRequest`/`StorageResponse` as internal protocol details of that adapter.
- No changes to `ofdb` are required; `KvClient.scanPrefix` already provides the listing capability.
- Serialization defaults remain JSON; ofdb KV values are JSON-native so the default path is zero-copy when records are JSON-serializable.

## Testing Decisions

- Test the `StorageBackend` interface contract with a mock in-memory backend: verify snapshot loading, mutation persistence, unacknowledged-write error, and refresh behavior are identical across backends.
- Test the ofdb adapter against a real `KvDatabase.open()` in-memory or IndexedDB (via fake-indexeddb in Node) instance: verify read/write/delete/list round-trips and that paths map to keys correctly.
- Test the WebSocket adapter to confirm no regression in existing behavior: same request/response mapping, timeout handling, and disconnect handling as before.
- Existing `index.test.ts` tests should pass against the WebSocket adapter unchanged; add equivalent tests parameterized over the ofdb adapter.

## Out of Scope

- Changes to `ofdb` Rust crates or the `@of/db` TypeScript wrapper API.
- SQL-backed collections (the collection is KV-shaped; SQL would need a different sync strategy).
- Multi-record atomic batches (the collection already rejects these; ofdb transactions could support them later but that is a separate feature).
- Real-time change subscriptions from ofdb (Committed Change Subscription) — the collection currently polls via refresh.

## Further Notes

- The `StorageBackend` interface should live in `storage-db-collection` itself (or a small shared types module) to avoid a new package dependency from `ofdb` back to `of`.
- The ofdb adapter could live either in `storage-db-collection` as an optional export or in a separate `@aicacia/storage-ofdb-adapter` package, depending on whether `storage-db-collection` should depend on `@of/db` directly.
