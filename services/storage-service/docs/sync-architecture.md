# Storage synchronization

Storage has separate native and WebSocket transports over owner synchronization protocols. This is a short source-checked summary, not an integration inventory or a current end-to-end pass claim.

## Current source behavior

### Native DATA replication

The native Storage runtime reacts to trusted policy, catalog, and peer-inventory changes. Database workers carry SQL and KV owner exchanges. Filesystem tasks synchronize metadata; content retrieval and Residency use separate authorized paths.

Policy invalidation fences both directions. Execution checks protect later reads, sends, receives, and applies on existing connections. See [native replication lifecycle](../../storage-server/REPLICATION.md).

### WebSocket SQL synchronization

The `sync` feature mounts a binary WebSocket route for Storage SQL synchronization. Its v1 framing carries a Hello/Attach handshake and opaque SQL v7 messages with Postcard encoding and CRC32 integrity checks.

The server requires a `storage:sync` access token and checks Resource, namespace, and deployment-specific selection policy. It rechecks authorization around transport and apply operations. The current server accepts only Bidirectional attach requests and runs a SQL responder. It does not run KV or filesystem synchronization over this endpoint.

Attach and backend session IDs are generated for the session. They are not durable local database identities. Attachment alone does not prove exchange completion, durable replica identity, or backup.

The Rust [Storage client](../../storage-client/README.md) contains native and browser SQL transports plus checked injected-stream SQL/KV APIs. TypeScript packages contain a raw WebSocket session and SQL owner adapter code. Their presence does not qualify every browser application flow.

## Target boundaries

In the accepted target, an Installation Device may hold authorized replicas for multiple Users and Applications. Resource Selection is desired Device replication state; Storage still enforces each Resource's `(User subject, Application)` namespace. Device revocation does not transfer Resource ownership or remotely erase copies.

The current legacy single-User Device owner/transfer model remains in parts of the code and contracts. Migration to shared Installation Devices is incomplete. See [target architecture](../../../docs/architecture.md).

Services consume only `ofdb-sql`/`ofdb-kv` in the target: `Database` for lifecycle/synchronization and `Client` for queries/transactions. Transport adapters must not import internal engine or redb crates. Current dependencies are not fully migrated.

Transport choice does not require one shared physical database or one database file per server process.

## Qualification

Protocol unit tests, an attached browser session, and a synchronized SQL result establish different facts. KV, filesystem metadata, and filesystem content require their own evidence. Do not infer backup or multi-Device authorization from a mounted route.

GitHub Actions records qualification results for each tested commit. Do not present a past run as current evidence.
