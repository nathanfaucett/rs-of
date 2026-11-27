# Storage synchronization

Storage has separate native and WebSocket transports over owner synchronization protocols. This is a short source-checked summary, not an integration inventory or a current end-to-end pass claim.

## Current source behavior

### Native DATA replication

The native Storage runtime reacts to trusted policy, catalog, and peer-inventory changes. Database workers carry SQL and KV owner exchanges. Filesystem tasks synchronize metadata; content retrieval and Residency use separate authorized paths.

Policy invalidation fences both directions. Execution checks protect later reads, sends, receives, and applies on existing connections. See [native replication lifecycle](../../storage-server/REPLICATION.md).

The Device API manages Selection for an approved Device and an exact owner, Application, Resource kind, and Resource ID. Management supplies the Application permission, not a separate list of Selections. Native replication checks both Devices against current Device and Selection state. Learned deselection denies later replication for that Device without deleting the Resource.

### WebSocket SQL synchronization

The `sync` feature mounts a binary WebSocket route for Storage SQL synchronization. Its v1 framing carries a Hello/Attach handshake and opaque SQL v7 messages with Postcard encoding and CRC32 integrity checks.

Both ordinary attach and first upload require a User bearer access token with audience and resource `storage:sync` and one Storage authorization detail with both `read` and `write`. Storage checks this capability, the Resource namespace, and applicable Device Selection at admission and during authorization rechecks. The server accepts Bidirectional and Pull attach requests and runs a SQL responder. It does not run KV or filesystem synchronization over this endpoint.

Attach and backend session IDs are generated for the session. They are not durable local database identities. Attachment alone does not prove exchange completion, durable replica identity, or backup.

A caller that needs first-upload admission must send `FirstUpload(AttachRequest)` (Postcard variant 7) instead of the ordinary `AttachRequest` after the v1 Hello. It uses the same request fields, Bidirectional direction, Resource checks, and `AttachResponse`. This does not change the v1 frame header, CRC, or SQL v7 `OwnerMessage` payloads. An ordinary attach keeps its established-replica behavior. Do not fall back to it if first upload fails or the server does not support the intent.

First upload accepts only a destination with no retained history; dropped schemas count as history. The SQL responder exchanges with a private empty database, not the live Resource. Publish rechecks the destination and installs the staged history in one database transaction. It rechecks authorization under the actual native writer lock just before commit. For non-seeded deployments, this uses the internal, authenticated Device Selection snapshot, not the endpoint that can repair policy or initiate owner cleanup. The final check reads current IdP and Device authority with cached service credentials only. A credential that needs renewal, an unavailable snapshot, an unresolved conflict, or required cleanup denies publish. Ordinary checks can renew credentials and reconcile policy before writer admission. Older Device owners without this operation do not support guarded publish; there is no fallback. Storage rechecks access-token expiry after the snapshot returns. Revocation, expiry, or a 30-second authorization deadline rolls back publish and returns `Abort(Unauthorized)`. The server sends `FirstUploadPublished` (Postcard variant 8, no fields) only after commit. Neither `AttachResponse` nor SQL v7 completion confirms publish. `Abort(DestinationHasHistory)` (Postcard bytes `04 04`) means the destination already had history at admission or at publish. A transaction conflict or other failure can also abort without publishing.

Retry on the same Resource with the same intent if the connection fails before publish. If the commit may have succeeded but the receipt was lost, another first upload fails closed when it finds that history. This protocol cannot distinguish a prior successful upload from independent history. Do not infer an established binding from this rejection.

For a remote-only restore, use ordinary v1 `AttachRequest` with direction `Pull` (tag 1). This direction needs only `read` on `storage:sync`; all namespace and applicable Device Selection checks still apply. Pull requires a live Resource and an existing physical SQL copy. It does not register, initialize, or update either. Admission and every later check use read-only Selection snapshots; admission can acquire service credentials if needed, but later checks never renew or repair. Storage serves SQL v7 history but rejects incoming history in both the main and recovery phases. No wire tags or frame formats changed. Older servers that do not grant `Pull` cannot restore; the browser does not fall back to bidirectional sync.

The Rust [Storage client](../../storage-client/README.md) contains native and browser SQL transports plus checked injected-stream SQL/KV APIs. TypeScript packages contain a raw WebSocket session and SQL owner adapter code. Their presence does not qualify every browser application flow.

## Target boundaries

In the accepted target, an Installation Device may hold authorized replicas for multiple Users and Applications. Resource Selection is desired Device replication state; Storage still enforces each Resource's `(User subject, Application)` namespace. Device revocation does not transfer Resource ownership or remotely erase copies.

The current legacy single-User Device owner/transfer model remains in parts of the code and contracts. Migration to shared Installation Devices is incomplete. See [target architecture](../../../docs/architecture.md).

Services consume only `ofdb-sql`/`ofdb-kv` in the target: `Database` for lifecycle/synchronization and `Client` for queries/transactions. Transport adapters must not import internal engine or redb crates. Current dependencies are not fully migrated.

Transport choice does not require one shared physical database or one database file per server process.

## Qualification

Protocol unit tests, an attached browser session, and a synchronized SQL result establish different facts. KV, filesystem metadata, and filesystem content require their own evidence. Do not infer backup or multi-Device authorization from a mounted route.

The password-manager reference scenario qualifies explicit single-Node encrypted-vault create, update, delete, and repeated synchronization through the public browser SDK and this WebSocket route. It checks remote ciphertext and plaintext/master-password absence. This does not qualify continuous delivery, two-Device convergence, durable replay, or other browser engines.

GitHub Actions records qualification results for each tested commit. Do not present a past run as current evidence.
