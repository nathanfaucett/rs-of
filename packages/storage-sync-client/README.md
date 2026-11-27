# @ofsdk/storage-sync-client

WebSocket transport for Storage SQL v7. This package does not open a local
`Database` or implement the SQL exchange. Apps with an already-open database
can use the [Sync SDK lifecycle](../sync-sdk/README.md) instead.

## Sessions

Construct `SyncClient` with the full HTTP(S) Storage API prefix and a token
provider. The client copies both at construction. For example,
`https://node.example/storage` becomes `wss://node.example/storage/sync/ws`.
Credentials, query strings, and fragments are rejected. Use HTTPS on untrusted
networks. The client obtains one token per attempt and sends it only in the
binary request. Storage requires a `storage:sync` token and checks the
Resource, namespace, and applicable Selection.

`StoreIdentity` contains a Resource UUID (`storeId`), a `replicaId` UUID, a
64-bit unsigned `creationEpoch` (`bigint`), and a 32-byte `schemaFingerprint`.
`session.result` describes attach acceptance and negotiated limits, not the
result of the SQL exchange. `send` and `receive` carry opaque SQL v7 bytes.

- `attach({ identity, direction, signal })` is the legacy attach interface.
  The Storage server requires `Bidirectional`. A direction flag alone does not
  change the SQL owner's behavior.
- `attachFirstUpload({ identity, signal })` sends the guarded bidirectional
  request. Run the SQL exchange, then call `await session.waitForPublish()`.
  The receipt is a Storage control frame, not an SQL message. An abort with
  `SyncError.code === "destination-has-history"` can arrive before acceptance
  or after exchange. Do not fall back to `attach` or infer publication from
  a lost receipt or a later abort.
- Pass an `AbortSignal` for cancellation and call `session.close()` in `finally`.
  The raw client has no deadline or retry policy. Buffered messages can be
  drained after remote close; close is not an acknowledgement.

## Wire contract

The source of truth is `services/storage-server/src/sync.rs`. Frames use
version 1, flags 0, a little-endian u32 payload length, a Postcard payload, and
its little-endian IEEE CRC32. Each binary WebSocket message is one frame. UUIDs
use Serde's binary byte-vector format; unsigned integers use Postcard varints.

First upload starts with the same Hello (0) and echo as legacy attach. It uses
FirstUpload (7) in place of AttachRequest (1), with the same fields. The
AttachResponse (2) and OwnerMessage (3) are unchanged. Storage sends
FirstUploadPublished (8) only after publish. DestinationHasHistory is Abort (4)
reason 4. Legacy wire tags and persisted data formats are unchanged. An older
server does not support first upload; failure does not trigger legacy attach.

## Checks

```sh
pnpm --filter @ofsdk/storage-sync-client build
pnpm --filter @ofsdk/storage-sync-client check
pnpm --filter @ofsdk/storage-sync-client lint
pnpm --filter @ofsdk/storage-sync-client test
pnpm --filter @ofsdk/storage-sync-client fixtures:check
```

Wire fixtures come from the Rust server types and `FrameCodec`. The fixture
script uses offline Cargo resolution. Use `--server-source` to validate against
another checkout of the server source while the slices are integrated separately.
