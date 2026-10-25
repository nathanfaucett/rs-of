# Storage DB collection

Framework-independent, eager filesystem collections with optimistic single-record CRUD for TanStack DB. Storage remains authoritative for persistence and authorization. This package has no Svelte, environment, OAuth Client ID, or Secret schema dependency.

```ts
import { StorageClient } from "@aicacia/storage-client";
import { createStorageCollection } from "@aicacia/storage-db-collection";

const client = new StorageClient({
  idpBaseUrl,
  storageBaseUrl,
  clientId,
  bearerToken,
});
const collection = createStorageCollection({
  client,
  userId,
  applicationId,
  filesystemId,
  directory: "records",
  schema: recordSchema,
  getKey: (record) => record.id,
});
await collection.preload();
// Consume with TanStack DB live queries / official bindings.
await collection.insert(record).isPersisted.promise;
await collection.update(record.id, (draft) => {
  draft.name = "Edited";
}).isPersisted.promise;
await collection.delete(record.id).isPersisted.promise;
// Dispose dependent queries before disposing their source.
await collection.cleanup();
```

The caller supplies a Standard Schema (synchronous or asynchronous validation), a stable string key, and authenticated User/Application/Filesystem ID scope. These scope labels identify the browser collection; they do **not** select or grant a server namespace. Storage derives ownership from the token. Create new clients/collections when the authenticated context or service bases change; never reuse a collection across identities.

## Installed API contract

Verified against installed `@tanstack/db 0.15.2` and `@tanstack/svelte-db 0.5.12` (the latter depends on core `0.15.2`). The lockfile fixes the qualified versions.

- `createCollection`, `getKey`, `gcTime`, and custom `sync.sync` are core APIs.
- Sync supplies `begin`, `write`, `commit`, `markReady`, and `markError`. We validate the entire snapshot before starting a sync transaction, await its commit receipt, then mark ready. Initial failures reject `preload()` and set the source status to `error`, never `ready`/empty.
- A sync cleanup function owns cancellation, listeners, and the socket. `cleanup()` clears source records. Adapter disposal is terminal, including disposal before the first load; attempting to preload again rejects without opening transport. Use a new collection to reopen.
- The password manager uses upstream `useLiveQuery` directly, including chained `orderBy`. Its reactive `data`, `isReady`, and `isError` are not destructured. No custom framework binding is required.
- `onInsert`, `onUpdate`, and `onDelete` receive `transaction.mutations` and persist through Storage. Await `transaction.isPersisted.promise` to observe success/failure, not just the synchronous optimistic change.
- Post-write sync uses `begin` / `truncate` / validated `write` / `commit`. Core accepts the snapshot before the handler returns, holding visibility behind persisting optimistic transactions. Do **not** await that visibility receipt inside the mutation handler: it depends on the handler settling. Core publishes the accepted authoritative state as optimism settles; tests assert server-normalized values and rollback through the public API. The adapter watches receipt rejection to fail closed.
- Operations on one collection are serialized through write, full reconciliation, and core settlement. Pending optimistic records are not dropped by an earlier snapshot. Rolled-back queued mutations are checked again before sending. Multi-record transactions are rejected before any Storage write; there is no server transaction or batch atomicity.

## Filesystem and failure semantics

Default representation is UTF-8 JSON at `<directory>/<key>.json`. Configure `extension` (a dot plus alphanumerics), `deserialize(Uint8Array)`, and `serialize(record): Uint8Array` together if required. Incoming and outgoing records pass schema and identity/path validation before publication or writing; serialization failure sends no write. Schema validation follows deserialization; validation errors do not include record content. Directories consist of safe, nonempty relative segments. Keys are single safe segments, must match the filename, and cannot contain separators, traversal, percent escapes, or trailing dots. Duplicate or mismatched identities fail the whole load.

The current Storage service returns **full relative paths** as listing entry names. It fails when listing a missing directory, so loading walks parent listings from the filesystem root. An absent configured directory in a successful parent listing is empty; a failed listing/read is never interpreted as empty. Unrelated extensions are ignored; matching entries with unsafe paths are rejected before reading them.

Read/validation failure publishes no partial snapshot and closes the owned connection. A disconnect/error after readiness puts the source into error; consumers must gate visible data on status. The adapter does not promise a server-atomic snapshot across multiple file reads. Concurrent external edits can fail a load or produce a mixed-time view.

`StorageClient.openSocket(filesystemId, signal?)` supports cancellation while obtaining a bearer token, exchanging it, and opening the socket. Disposal prevents later requests (including queued writes) and ignores late reads or schema results. A write already sent may still persist: disposal is not server-side cancellation. Its completion cannot publish into a replacement collection. Always clear obsolete views and await/catch mutation promises; do not display a completion belonging to an old identity or filesystem. There are no timers, background retries, polling, push change feeds, or automatic reconnects. `gcTime: Infinity` means the owning application **must** call `cleanup()`, not just unsubscribe.

The current Storage client requests read/write socket authorization. Discovery does not grant resource access. A denied/expired token or unavailable content is an error. Browser-offline persistence, decrypted-secret caching, and mutation outboxes are not implemented. Node replication is independent of browser collection freshness.

## Mutation outcomes

- An expected `written` acknowledgement for the requested path, or `deleted`, is followed by a complete authoritative reload before the handler settles. The authoritative record may differ from the submitted value, or another client may have changed/deleted it. Storage has no compare-and-swap contract; this is not conflict prevention or a server-atomic snapshot.
- An explicit Storage error also reloads before rejecting; optimism rolls back to that authoritative state, not a fabricated pre-write copy. Review the resulting records before retrying.
- Missing, malformed, or unexpected acknowledgement (including the wrong write path) triggers reconciliation. If it succeeds, `UnacknowledgedStorageWriteError` still rejects the transaction even when the write appears in Storage. Verify the visible outcome; do not blindly repeat a create or assume failure means nothing persisted.
- A disconnect or failed/malformed reconciliation fails the collection closed, closes the owned socket, and rejects the mutation with an uncertain-outcome warning. Hide obsolete records, construct a new collection, and verify Storage before retrying. No automatic retry occurs. The current transport has no request deadline; a silent connected endpoint can keep a write pending until it responds, disconnects, or the owner disposes it.
- Explicit Refresh/reconnect-in-place is ticket 04. Reopening a new collection already reloads persisted CRUD.

## Checks

From the workspace root, install dependencies and build `@aicacia/idp-client` and `@aicacia/storage-client` with their `build:esm` scripts first. Then:

```sh
pnpm --filter @aicacia/storage-db-collection test
pnpm --filter @aicacia/storage-db-collection check
pnpm --filter @aicacia/storage-db-collection build
pnpm --filter @aicacia/storage-db-collection lint
pnpm --filter @aicacia/storage-db-collection format
```

Tests use real TanStack collections/live queries and the real Storage client with controlled HTTP/WebSocket responses, not a substituted DB or mocked collection implementation. They qualify client behavior, not a deployed Installation, server authorization, or cross-Node replication.
