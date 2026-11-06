# @ofsdk/sync-client

Browser WebSocket attach and transport for the Storage server SQL sync endpoint.
No runtime dependencies. This package does **not** implement the ofdb SQL sync
engine, read a local database, or prove that data was backed up.

## Interface for app integration (ticket 02)

```ts
import { SyncClient, type StoreIdentity } from "@ofsdk/sync-client";

export function attachStore(
    storageApiUrl: string,
    identity: StoreIdentity,
    getSyncAccessToken: () => Promise<string>,
    signal: AbortSignal,
) {
    const client = new SyncClient({
        storageApiUrl,
        tokenProvider: getSyncAccessToken,
    });
    return client.attach({ identity, direction: "Push", signal });
}
```

- `storageApiUrl` is the complete HTTP(S) Storage API prefix. For example,
  `https://node.example/storage` becomes `wss://node.example/storage/sync/ws`.
  Custom prefixes and trailing slashes work. Do not pass the WS endpoint itself.
  Credentials, query strings, and fragments are rejected. Use HTTPS on untrusted
  networks: HTTP sends the token over an unencrypted WebSocket.
- Supply a current IdP token through `tokenProvider`; it is called for each
  attach. The current server requires audience `storage:sync`, active identity,
  and a selected resource for its endpoint lookup. An ordinary sign-in token
  or Storage resource token must not be assumed sufficient. Tokens are sent
  only in the binary AttachRequest, not URLs, headers, logs, or errors.
- `StoreIdentity` contains `storeId`, `replicaId` (UUID strings),
  `creationEpoch` (unsigned 64-bit **bigint**), and `schemaFingerprint`
  (32-byte `Uint8Array`). Use the real persistent owner identity. Do not generate
  a new replica identity each run or use dummy fingerprints for a real backup.
- Directions are `Push`, `Pull`, and `Bidirectional`. The immutable
  `session.result` reports `status: "attached"`, `requestedDirection`,
  `grantedDirection`, backend/session UUIDs, and effective frame/session limits.
  A grant that cannot permit the requested direction rejects attach.
- `session.send(payload)` and `await session.receive()` carry **opaque
  ofdb SQL v7 owner messages**, not SQL text, JSON, files, or database dumps.
  They are the transport boundary for a compatible owner sync engine.
  The owner engine must implement the requested direction; selecting `Push`
  here does not implement send-only database synchronization.
- Pass an `AbortSignal` for cancellation, including stalled token acquisition,
  opening, attach, and attached sessions. There is no implicit retry or deadline.
  The caller can use its existing timeout policy. Call `session.close()` in a
  `finally` block. Pending receives reject on close, failure, or cancellation.
  Complete buffered messages can be drained after remote close, but remote close
  is not an acknowledgement of owner commit or backup completion.
- `SyncError.code` is `auth`, `protocol`, `transport`, `unavailable`, or
  `cancelled`. Auth rejection is distinct from backend failure. Do not persist
  tokens, bigint-containing raw results, or WebSocket handles in app state.
  Convert selected bigint metadata to decimal strings if persistence needs it.

Ticket 02 must report **attached**, not **backed up**, until a compatible local
owner engine completes its full exchange/apply/acknowledgement contract. This
package does not supply a browser SQL engine or filesystem/KV sync. It does not
alter Device selections, enroll Devices, probe capabilities, persist app state,
or provide mesh sync. Add the workspace dependency to the app when integrating.

## Actual server contract and limits

The source of truth is `crates/storage-server/src/sync.rs`, not the existing
Rust `sync-client` or `sync-test-harness`: their message enums differ from the
server. Frames use version 1, flags 0, little-endian u32 payload length,
Postcard payload, and little-endian IEEE CRC32 of that payload (not CRC32C).
UUIDs use Serde's binary byte-vector form; unsigned integers use Postcard
varints. Each binary WebSocket message contains one complete frame.

The client sends Hello then AttachRequest. The current server echoes Hello;
this is not protocol negotiation. AttachRequest has **no reconnect field**.
Server message tags are Hello=0, AttachRequest=1, AttachResponse=2,
OwnerMessage=3, Abort=4, Ping=5, Pong=6. Unauthorized is Abort reason 0.
Unsupported flags, malformed/trailing bytes, CRC errors, and unsupported owner
protocols fail closed. Payloads are capped at 16 MiB, and cumulative sent/received
wire bytes (including handshake) at the smaller of 1 GiB and the server limit.
These are protocol bounds, not hard browser memory/CPU isolation guarantees.

The current server always grants Bidirectional, uses endpoint lookup `local`,
and starts a default SQL responder session. It does not enforce the requested
Push/Pull direction in `run_sync_session`. Attach acceptance occurs before the
server opens the selected database, so even an accepted attach can be followed
by immediate close. This package preserves those distinctions; it does not fix
server authorization or direction behavior and does not claim live-server,
resource-transfer, durable-backup, or filesystem qualification.

## Checks

```sh
pnpm --filter @ofsdk/sync-client test
pnpm --filter @ofsdk/sync-client build
pnpm --filter @ofsdk/sync-client lint
pnpm --filter @ofsdk/sync-client format
pnpm --filter @ofsdk/sync-client fixtures:check
```

Protocol tests use an in-memory WebSocket and checked-in wire fixtures generated
by **the actual Rust server types and FrameCodec**. `fixtures:check` compiles
those definitions in a temporary standalone Cargo project and detects drift.
It uses offline Cargo resolution and needs cached serde, uuid, postcard, and
crc32fast packages plus a Rust toolchain. `pnpm --filter @ofsdk/sync-client
fixtures` regenerates the fixtures; review changes against server behavior.

Change assessment: small isolated package; moderate wire-parser/auth risk,
covered at the public client/session boundary. No existing data or callers are
changed. Runtime dependencies: none. App integration remains ticket 02.

## Data-engine work required for ticket 02

1. **SQL owner binding.** `ofdb/crates/sql/src/database.rs` exposes
   `Database::synchronize_checked` (memory staging, including IndexedDB) and
   `synchronize_with_stage_directory_checked` (native staging). The browser
   owner tests in `ofdb/tests/browser/src/sql_sync.rs` open persistent databases
   with `Database::open_indexeddb`. Publish a JS/WASM owner binding, not a second
   SQL sync implementation. Its `ofdb_sql::SyncTransport` adapter delegates raw
   owner `send(Vec<u8>)` / `receive()` to this session. Run as Initiator; keep
   the database identity, cancellation, and pre-apply checks with the owner.
   Return the owner `SyncResult` only after the full exchange succeeds. Earlier
   atomic applies can remain after a later failure; do not promise rollback.
2. **Direction contract.** `ofdb/crates/sql-sync/src/session.rs` rejects
   non-Bidirectional unscoped sessions. Scoped sessions require complementary
   directions (`SendOnly` / `ReceiveOnly`). The WS server currently uses default
   unscoped Bidirectional config regardless of the attach request. Merely mapping
   Push to SendOnly in the client is not enough: the server and owner scope
   contract must be implemented together before promising push-only backup.
3. **KV and filesystem coverage.** App collection adapters operate on KV/file
   records. The WS endpoint only starts the SQL engine. By contrast,
   `database_protocol.rs` runs SQL then `ofdb_kv::Database::synchronize_checked`
   over iroh. Filesystems use `storage_protocol.rs` and
   `FileSystem::metadata_sync(...).with_operation_control(...)`; the `offs`
   owner also fetches file content through `FileSessionService`. Metadata alone
   is not a full-content backup. Add authorized WS owner dispatch or use a
   native host driver for the existing iroh path. Do not send KV/file bytes as
   `OfdbSqlV7` messages.
4. **Existing adapters are not an engine.**
   `packages/storage-db-collection/src/adapters/sync.ts` contains an unused
   placeholder with JSON in place of Postcard, CRC32C, token URL parameters,
   and unimplemented CRUD/sync. It is not exported by the package root. Do not
   use it for backup or as the wire reference. Replace/delete it when that
   adapter is integrated; no compatibility layer belongs in this ticket.
   The Rust native/browser clients also have incompatible attach enums and
   bridges that do not unwrap the server OwnerMessage envelope.
5. **Admission and completion.** Obtain a current token with `storage:sync`
   audience through the approved IdP flow. Token exchange uses `resource` as
   the requested audience in `crates/idp-service/src/oauth2/service.rs`; verify
   the app OAuth Client allows exchange. Resolve the server hard-coded `local`
   endpoint against real Device selections. The current WS pre-apply callback
   always returns success, unlike the iroh authorization wrappers; changing
   admission/revocation checks needs server work. Attach succeeds before opening
   the database, and its replica UUID is generated per attach, not read from the
   database owner. Use real owner completion and identity for data status.

Suggested next test: open two real owner databases, write known data in one,
attach through the real router, run the owner protocol through this session,
and read the committed data from the other after reopen. Add KV and filesystem
content cases before the UI reports that all selected resources were backed up.
