# Design

## Overview

The `ofdb` workspace contains separate SQL and KV stores. SQL engine writes commit schema/catalog, row state, tombstones and derived indexes atomically. Replication is transport-neutral and implemented by each store's sync crate; peer discovery, resource authorization and network framing are caller responsibilities.

## SQL engine

Catalog DDL and sync use the ordinary row codec, transaction, and tombstone paths described below; the parallel schema-change and catalog-entry protocols have been removed.

The engine owns canonical database state and exposes transactions to local callers and sync. The four internal catalog tables (`tables`, `table_fields`, `indices`, `index_fields`) are the sole source of SQL schema and can be queried directly through SQL. `db/sql-engine/src/schema.rs` owns their storage names, plain value-row structs, and fixed built-in row layouts, and converts queried rows into `TableSchema` and `IndexSchema`. Table and index definitions are ordinary catalog rows with independent UUIDv7 primary keys; competing definitions for one name select the greatest row UUID. Tables are identified by name, and derived index storage uses the index name directly, not a UUID or internal prefix. The four catalog names are reserved for both tables and indices; other table and index names share one storage namespace, including historical cross-kind name claims. Former `__engine_` names have no special reservation or compatibility behavior. The engine maintains derived indexes atomically when local or incoming state is applied. Dropping a table deletes its contents and dependent schema definitions; deleting a losing definition does not drop the named table. An index name can be reused with a new definition, and cleanup of an inactive definition preserves storage owned by an active same-name index. Existing databases using the old catalog and index storage names require fresh storage or manual migration; there is no automatic compatibility layer.

The Automerge SQL engine generates every Logical Row identity as RFC UUIDv7. For a single `UUID` primary key, omit the key or use `DEFAULT` and read the generated key with `RETURNING`. `INSERT … DEFAULT VALUES` inserts one row with a generated key and the other column defaults. Both embedded and remote Clients reject explicit keys, including explicit UUIDv7; `NULL` is not a default. UUIDs remain valid as ordinary data and as components of composite keys. Natural and composite keys use a separate generated Logical Row identity. Replication preserves peer UUIDv7 identities without a wire change. Initialize fresh stores: legacy UUIDv5 Logical Rows and histories are unsupported, and there is no migration. The independent PostgreSQL backend is unchanged.

Concurrent row values may be retained as conflicts by the Automerge row codec. Deterministic canonical ordering determines the visible value; an explicit resolution settles a conflict. Deleting a table or index marks its current catalog row with the ordinary row-codec tombstone, never a `deleted` value column, through the same codec and sync path as a user row. A tombstoned greatest UUIDv7 keeps the logical name absent; tombstones participate in winner selection but do not appear in search results. An older live row or stale snapshot cannot restore it. Recreating a name creates a new, greater UUIDv7 row instead of clearing the tombstone. This needs no separate schema-version table, schema tombstone protocol, or compatibility path. Old database files must be discarded and recreated after the format changes.

## Native SQL open and bounded reads

`Database::open` may initialize missing or empty native persistence. Owners reopening acknowledged resources must use `Database::open_existing`, which validates an existing file without creating or initializing it. Both retain the permanent retirement fence.

`Client::transaction()` exposes `Transaction::set_read_limits(entries, bytes)` for native Redb transactions. Configure it once, before execution. The cumulative native budget counts key/value bytes and entries across catalog reads, repeated scans, and old-value mutation reads. Borrowed native encodings are checked before owned key/value decoding or range accumulation. A separate cumulative decoded scalar-row budget uses the same limits for SELECT scans, mutation matching, and inserted/updated rows. It accounts for row/value storage plus text/blob payloads (including empty and defaulted fields), and is checked before scan/result accumulation or mutation encoding, including rows later excluded by WHERE, LIMIT, or OFFSET. JSON/Type decoded rows are unsupported under this budget. Exhaustion returns structured `ErrorKind::LimitExceeded` and aborts the SQL transaction; commit cannot retain prior mutations. Remote and non-native backends fail closed with Unsupported when configuring this native-only facility. Ordinary clients remain unlimited unless explicitly configured.

These are read/materialization budgets, not a hard process-memory, decompression, instruction, wall-clock, or concurrency quota. A codec must decode one row/document before its logical size is known; native encoded reads are bounded before owned allocation. Catalog reconstruction, codec history, and transient encodings have overhead. Callers exposing untrusted SQL must additionally reject data-amplifying shapes (joins, subqueries, computed/repeated projections, and unsupported DDL) before transaction admission. Limits alone do not make unrestricted SQL a safe HTTP workload. The Storage caller owns that restricted SQL contract, authentication, and wire-response limits; no HTTP or application-vault policy belongs here.

## SQL sync boundary

The SQL facade's `Database::synchronize*` methods coordinate a peer session over caller-provided `SyncTransport`. `SyncMessage` protocol version **7** is incompatible with earlier versions; no legacy wire adapter is retained. Selected endpoints first negotiate the union of `SyncSelection::{All, Selected { tables, rows }}`; either endpoint's All makes discovery broad. Both endpoints must use selected negotiation. The session then exchanges:

- hello and a digest manifest;
- row-change inventories;
- mismatched canonical state units for bootstrap and recovery;
- incremental changes identified by actual native document change IDs;
- requests for snapshots when dependencies are missing; and
- completion or abort.

`SessionConfig::max_units_per_frame` bounds frame item counts; SQL messages and complete apply batches are bounded to 1 MiB, and `max_session_bytes` defaults to 64 MiB. Oversized work fails explicitly; there is no chunked single-row protocol. Transports must bound incoming reads before buffering: session size checks happen afterward. The transport supplies ordered byte frames, not identity or authorization. Callers authenticate and authorize the resource before invoking sync and use checked APIs for changing authorization/cancellation.

Discovery obtains engine-owned `DocumentSummary { digest, metadata, changes }` through `RowReconciler` in one read transaction. The Automerge reconciler reconstructs history without saving/reloading a snapshot just to calculate a digest. SQL sync owns selections, manifests, wire units, staging and retry; `SyncRowCodec` is a direct re-export of the engine trait, not a second interface. The production Automerge reconciler does not depend on SQL sync. Native inventory/export includes snapshot-contained and bundled changes; physical `DocumentChangeKey` labels are not native change IDs. Payload identities and dependencies are validated before writes; engine-managed rows still use validated engine mutation and atomic derived-index maintenance.

Ordinary metadata-free updates send missing native changes. Bootstrap, missing/unavailable dependencies/history, and mismatches with nonempty metadata use complete state plus metadata. Thus metadata-only deletion and combined history/deletion changes cannot be lost by selecting deltas. Recovery snapshots merge concurrent histories rather than replacing local work. Selected exact-row discovery avoids unrelated row-ID enumeration; table invalidation permits table scans, and catalog/dependency or unknown invalidation promotes discovery to All. Broad comparison still defers snapshot export until needed.

A catalog change transfers its affected table/index dependency set across the four catalog tables as one logical batch, not the entire database catalog. The index definition consists of its index row and all index-field rows in that complete batch; no separate expected-field count is stored. The batch can span transport frames; complete applied batches commit atomically, not the entire session. An interrupted catalog batch is discarded and retried through state exchange; dependent user-row changes wait for its commit. Independent columns with the same position are ordered deterministically by name. A catalog batch with a known-invalid definition is rejected atomically with an actionable error. Independently created columns with the same name under one table identity have distinct UUIDv7 identities; the greatest identity selects the visible definition. The engine has no envelope log, causal frontier, checkpoint, quarantine, or transport state.

### Independent history staging

The SQL facade captures each source in one read transaction and reconciles the two histories in private memory. Staging requires matching active table and index schemas and a record policy for each table. It checks the exact primary key and caller-specified stable fields through retained history, including deleted rows. It rejects duplicate keys across Logical Rows and requires each visible encrypted field group to match one live source row. It rejects unresolved conflicts in split field groups. Tombstones remain in the candidate.

The migration ledger accepts identical history or unedited, independently applied copies of the same versions, descriptions, and fingerprints. It retains both Logical Rows and tombstones the redundant copy in the candidate. The application must still check its migration plan. Histories that need broader ledger reconciliation are rejected.

A candidate reopened for inspection has no binding. Staging proves field provenance, not authentication, parent-child integrity, or source freshness. It cannot approve publication. Publication would have to verify both snapshots under their respective write fences. The stored format and SQL sync v7 do not change.

## KV store and sync

KV is a separate store of shared `ofdb-value::Value` values under UTF-8 keys. Values include primitives, blobs, and nested JSON objects and arrays. Each key has UUIDv7 generations with Automerge-backed history and tombstones. Updates reconcile values inside the Automerge document, so concurrent nested-map/list edits merge. Concurrent edits to one scalar may retain an Automerge conflict, with deterministic visible-value selection. A tombstone wins a concurrent delete/update race; setting a deleted key creates a new generation. Exchange is explicit through `kv-sync`; transport and peer state belong to its caller. The generation with the greatest UUIDv7 determines visible state. The API currently exposes shared value and JSON types, not every native Automerge object type (for example, collaborative text or counters).

KV wire **3** is incompatible with earlier versions. Broad and selected sessions compare native history inventories and exchange missing changes for matching generations; bootstrap/new generations and dependency gaps use snapshots. Selected negotiation compares the union of logical keys and re-reads latest generations: an invalidated old generation is not an export filter. Native hash and exact dependency claims are checked against payloads. A gap rolls back the whole incoming batch before snapshot recovery; recovery must contain promised changes or a newer generation. Known history causes no writes. Earlier complete batches survive a later failure.

`KvTransaction::{history, histories, export_changes, import_changes}` owns history validation and merge semantics. Inventory/export reconstructs selected documents and enumerates their full history, not only recent deltas. A document inventory or delta and aggregate batch must fit configured frame/count budgets; no single-document chunking is provided. Broad fallback retains a whole-store inventory. KV Expiry is read-time visibility, not a timed mutation or sync wakeup.

## Committed selection and peer lifetime

Backend transactions publish invalidation only after successful persistence, synchronously before releasing writer admission (Redb), or under the successful optimistic-commit state lock (SQL memory). Rollback, failed persistence, conflicting memory commits and net physical no-ops publish nothing. Incoming replication uses the same boundary for fan-out; physical compaction may harmlessly invalidate without changing logical history. Derived index bytes are not independent replicated identities.

One signal retains at most **128 commits**, each capped at **128 physical invalidations / 16 KiB of identity bytes**, with oversized commits represented by All. A selected token can combine retained commits; these are retention limits, not total session or transaction-memory limits. Each consumer independently selects work before opening reconciliation reads. Acknowledge only its captured watermark after successful reconciliation; later commits remain pending even if visible during that pass. Error/cancellation drops the token without acknowledgement and retries pending work. `changed()` consumes coalesced wakeups, not selected work. Wrong-consumer or pre-merge tokens are rejected; tokens must also be mapped to their corresponding store/backend.

Keep a separate, unmerged subscription per store/peer relationship. New subscriptions and clones require All; recreate subscriptions on reconnect/restart or peer changes rather than carrying an old peer's frontier forward. Ring gaps/overflow and unknown writes require broad comparison; after success, selective work resumes. SQL maps physical identities to rows/tables/catalog invalidation; KV maps key/generation identities to latest-key selection. Neither retains document payloads or a durable log.

IndexedDB physical signals cover handles for the same store in one WASM runtime, not other tabs or workers. The browser SQL facade exposes coalesced wakeups without history payloads. Redb signals cover shared backend wrappers, not independently constructed wrappers over the same raw database or external writes. SQL memory supports subscriptions under std; generic KV memory does not (`Unsupported`), so use explicit broad sessions. Unsupported kernels likewise require caller-driven broad reconciliation.

**Local notifications cannot observe commits made only on a remote peer.** Callers supply authorized remote wakeups/receive delivery or explicitly invoke both sides of reconciliation, including with empty local selection. Remote pending selection contributes work through negotiation; if it cannot be supplied reliably, request All. Local runners alone do not promise bidirectional realtime delivery. Discard transports after errors and retry with fresh transports. A session is neither globally atomic nor a globally frozen snapshot; complete-row/catalog or KV-batch atomicity and valid earlier committed progress remain intact.

## Measured discovery work

Deterministic fixtures count public reconciler/backend operations, not elapsed time or every private reconstruction in validation/catalog/index work:

| Established one-row SQL edit, both peers combined | Original broad baseline | Selected path |
| ------------------------------------------------- | ----------------------: | ------------: |
| Full snapshot export calls                        |                      49 |             0 |
| Separate history inventory calls                  |                      26 |             0 |
| Nonempty snapshot digest reloads                  |                      48 |             0 |
| Direct history summary reads                      |                not used |             2 |
| Table/row-ID discovery scans                      |                   broad |             0 |
| Native change frames from source                  |                       1 |             1 |

`sql-engine-automerge/tests/change_detection.rs` asserts the selected budgets and combined metadata/deletion convergence. The KV fixture in `kv-sync/tests/selective.rs` establishes 101 keys per peer, edits one selected key, and asserts four total directional inventory entries, no snapshots and fewer than 100 physical record reads per peer. Replay sends no data. These fixtures demonstrate selected discovery reduction, not constant work for arbitrarily large document histories. Broad fallbacks and selected-history enumeration retain the costs described above.

## Facades and CLI

The public facades are `ofdb-kv` for KV and `ofdb-sql` for SQL. Facades live in `db/sql` and `db/kv`; the KV implementation lives in `db/kv-store`. SQL and KV have separate client binaries, `sql-client-cli` and `kv-client-cli`, and separate server binaries, `sql-server-cli` and `kv-server-cli`. They share a publishing approach, not a data model or storage interface.

SQL-specific crate directories use `sql-*`; their Cargo package names use `ofdb-sql-*`. This includes engine, engine-automerge, engine-redb, query, schema, client, server, proto, protocol, sync, value, macros, and test. The `sql-translator` package is `ofdb-sql-translator`. Shared B-tree package names use `ofdb-btree*`, without a SQL prefix. KV implementation/support packages use `ofdb-kv-*`, with `ofdb-kv-store` distinct from the public `ofdb-kv` facade.

## Ownership boundaries

`ofdb` (in `db/`) owns SQL/KV persistence, CRDT sync, staging, batching, and cleanup through public `ofdb-sql`/`ofdb-kv` only. `of` owns domain identity, policy, catalogs, endpoint-to-Installation-Device trust, local authorization, and protocol-specific transport adapters wiring persistence owners. Generic tests belong to these owners; `of` tests authorization integration. Each Node shares one `iroh-chain` endpoint/router, with endpoint identity and locally trusted membership persisted and restored by `of`. Allowlisting is not resource authorization; pairing is not general access. Mesh messages and direct streams are distinct, with no transparent multihop database/filesystem sync claim. See [networking and synchronization](../docs/architecture.md#networking-and-synchronization).

### Apply authorization and staging

Database open, write and sync invocation require validated Installation tokens. `of` enforces token audience, scope and expiration; `ofdb` receives a caller-validated `Client` and executes without further policy checks. Staging (memory, caller-provided, optional native Redb) is an `ofdb` implementation detail, not an `of` policy surface.

Shared SQL sync staging permits at most 128 live stages and pending cleanups per process. One worker removes tables when a stage cannot acquire writer admission immediately; dropping a stage does not wait for an authorizing writer.
