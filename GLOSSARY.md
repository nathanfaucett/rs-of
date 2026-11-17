# Glossary

## Shared domain

### Language

- **Installation** — The top-level deployment identity. An Installation has one IdP signing key and one set of Applications. Nodes join an Installation through a trusted peer; the Installation issuer string is the same as the IdP base URL plus `/idp`.
- **User** — A human or service identity registered with the IdP. Users own credentials, consents, and application-scoped resources.
- **Application** — A logical grouping of OAuth clients that share user resources. An Application has a URI and one or more Clients.
- **Principal** — The authenticated actor in a request: a User, a Client, or a Device. The IdP issues access tokens for Principals.
- **Device** — An approved endpoint-ID binding with a User owner, managed by Device service. A Device is the authorization boundary for synchronization and Storage access.
- **Setup lifecycle** — Create (generate new Installation state), Join (learn Installation state from a trusted peer), Reset (remove local Installation state). Each owner retains its own setup surface.

### IdP

- **Credential** — A secret or public-key method for authenticating a User. Credentials are local to the IdP and never replicated.
- **Client** — An OAuth client configuration: client_id, redirect URIs, allowed scopes, secret hash. Clients belong to an Application.
- **Token** — An opaque string issued by the IdP (access token, refresh token, ID token, authorization code). Tokens carry audience and scope; they do not authorize peer synchronization.
- **Signing key** — The Installation's asymmetric key pair for signing tokens. The private key is local to the IdP; the public JWK is published at `/.well-known/jwks.json`.
- **Replica signer** — A node-scoped signing key enrolled through IdP, bound to an endpoint ID. Used for node-to-node authentication, not User-facing tokens.

### Management

- **Role** — A named collection of permissions. Roles are assigned to Users or Applications.
- **Permission** — An atomic action on a resource (e.g., `idp.security.sync`, `management.devices.read`). Permissions belong to Management; other owners expose their permissions through Management assignments.
- **Assignment** — A grant of a Role to a Principal for a specific resource (Installation, Application, Device).

### Device

- **Endpoint ID** — The Iroh public key that identifies a Node's network identity. A Device binds one Endpoint ID to one User.
- **Approval** — The Device service action that creates or revokes a Device record. Approval is the only way a Device becomes authorized.
- **Selection** — A Device's explicit choice of which resources (databases/filesystems) to replicate. Selection is whole-resource and per-Device.
- **Restriction** — Administrator-defined limits on Device selections (e.g., max devices, required residency).
- **Transfer/Revocation** — Moving Device ownership to another User, or permanently removing a Device's authorization.

### Storage

- **Resource** — A named or unnamed database or filesystem with a stable identity. Resources belong to a `(user subject, application_id)` namespace.
- **Catalog** — The Storage service's index of resources for a namespace.
- **Selection (Storage)** — The Storage API parameter that identifies a specific resource for read/write operations.

### File System (offs)

- **KV Store** — A key-value store holding values identified by keys, with retained generations and deletion tombstones. Reconciliation combines histories within a generation and selects the winning generation for each key.
- **File System** — Offline-first replicated storage of paths, file metadata, and content. Filesystem reconciliation brings replicas together while retaining stable file identities, deletion tombstones, and device-local residency choices.
- **File Entry** — The metadata for one path: stable file id, kind, providers, locality, mode, owner, group, and content revision. Its file id survives overwrite and rename; a deleted path is represented by a tombstone.
- **Content Revision** — Identifies a file's metadata/content version, independently of its metadata generation. It distinguishes the version requested or edited from stale versions.
- **Residency** — A device-local choice per path: Full retains metadata and content, while read-only Passthrough retains metadata and obtains content from an available Full peer. The most-specific choice applies, and residency is not replicated.
- **Transport** — The caller-supplied exchange of filesystem metadata and file-session requests between peers, not filesystem persistence or reconciliation. A File Session is a peer-scoped exchange for opening, reading, writing, scanning, and closing a file handle.
- **File System Resource** — A named or unnamed filesystem with a stable identity. Resource deletion is a replicated tombstone; device deselection removes only that device's projected local copy.

### Endpoint Mesh (ofnet)

- **Endpoint** — A uniquely identified participant in the network that can communicate with other endpoints. Prefer this term over node or server when referring to the participant rather than its hosting process.
- **Peer** — An endpoint known to another endpoint as a possible mesh neighbor. A peer is an identity; a connection is a relationship between peers.
- **Mesh** — The network of peer relationships through which endpoints communicate, including communication relayed by intermediate endpoints. Prefer this term over overlay network unless discussing the underlying networking concept.
- **Broadcast** — A message intended for every endpoint in the mesh. Prefer this term over multicast when the intended audience is the whole mesh.
- **Direct Message** — A message intended for one identified endpoint, whether reached directly or through relays. Prefer this term over point-to-point, which can imply a direct connection.
- **Byte Stream** — An ordered flow of bytes between two endpoints, without message-level interpretation by the mesh. Prefer this term over message stream.

## Database (ofdb)

### Facade Roles

- **Database (Control Handle)** — Manages storage lifecycle (`open`, `in_memory`), hosts remote endpoints (`serve_tcp`, `serve_unix`), and runs sync sessions (`synchronize`). Does not execute queries.
- **Client (Query Handle)** — The sole interface for reading and writing. Get an embedded client via `database.client()` or connect remotely. Same API for local memory, on-disk Redb, or network.
- **Transaction** — Explicit multi-statement transactions with atomic `commit()` and `rollback()`. Remote transactions stream over one connection and roll back if the client handle drops.

### Query Operation

A Query Operation is a store-specific read or write request. SQL operations execute statements; KV operations read or change shared `ofdb-value::Value` values under UTF-8 keys. KV values can be primitive values, blobs, or nested JSON objects and arrays; Automerge merges concurrent changes to nested values. SQL and KV do not share a query interface. Each store has structured query errors shared by its embedded and remote query interfaces. Remote requests preserve store-specific categories through structured error details, not message matching. An empty SQL statement batch is invalid, not a successful no-op.

### Embedded Handle

An Embedded Handle, named `Database` in each facade, constructs and controls a database in the caller's process. It provides explicit sync and hosting setup, but is not the facade query or transaction API. Callers use `Database::client()` for embedded queries and SQL transactions. Callers do not supply query timestamps or timestamp providers; explicit KV expiry settings remain supported.

### Query Handle

A Query Handle, named `Client` in each facade, is the only facade API for ordinary queries against embedded storage or a database hosted by another process. It includes reads and writes. SQL and KV clients create explicit transactions for embedded and remote databases. Clients do not provide hosting or sync setup. SQL and KV clients remain separate types with separate query interfaces.

`Client` supports embedded and remote queries. `Database::client()` is the only embedded client constructor. It creates an owned handle that shares storage and remains usable after the `Database` handle is dropped. Storage construction stays on `Database`; remote connection constructors stay on `Client`. `Database` does not expose duplicate query methods or SQL transaction creation. Both CLIs use `Client` for query dispatch.

A remote client's async connection constructor establishes a connection immediately. `Client::transaction()` establishes an SQL transaction for embedded or remote storage. Remote transactions use one server-side transaction stream across calls; dropping an uncommitted transaction rolls it back when the server observes stream closure. `Client::with_deadline(Duration)` applies to queries and transaction operations, with no default. Timeout does not prove rollback or guarantee interruption of blocking storage work. The CLI retains one budget for construction plus query.

Clients support `Clone`. Embedded clones share storage; remote clones share the connection. Each clone keeps its own deadline setting. SQL `Client` exposes typed statement execution, SQL-gated `execute_sql()`, generic translator helpers, and explicit transactions. KV `Client` exposes its existing get/set/delete/scan operations and explicit transactions. A transaction holds local operations until commit; a remote transaction uses one server-side stream and rolls back when an uncommitted client handle is dropped and the server observes stream closure. `Database` does not expose ordinary query methods or transaction creation. SQL-text queries use one deadline budget for translation plus execution. Library query deadlines do not include connection establishment. Browser embedded queries, with or without a deadline, do not require a Tokio runtime. Native embedded queries with a configured deadline require a Tokio runtime with time enabled; untimed embedded queries remain usable without Tokio. A transaction inherits its client's deadline setting, with a fresh budget per operation rather than a transaction-lifetime limit.

### Database Host

A Database Host owns the embedded database used to answer remote query requests. Sync setup belongs to the host or embedded caller, not to remote clients.

### Committed Change Subscription

A Committed Change Subscription is an independent consumer of bounded, process-local invalidation for successfully committed mutations, including incoming replication. Its wakeups are not durable replay or evidence of remote-only writes.

### Sync Session

A Sync Session exchanges replicated state between stores of the same kind using caller-supplied transport, peer selection and authorization. SQL and KV retain separate protocols; their continuous runner owns change-driven scheduling and retry when explicitly started, without automatically spawning background tasks.

### CLI

Two separate executables, `sql-client-cli` and `kv-client-cli`, use their corresponding published facades for embedded or remote queries. They are one-shot query clients only: neither exposes hosting, sync, administration commands, or an interactive shell. Each invocation selects exactly one explicit target: database path, memory, endpoint URL, or Unix socket path. An explicit database path may create a missing database; no default path is selected or created. SQL input is one query argument or one file/stdin input executed as a single statement batch. KV CLI values use an explicit tagged JSON encoding; files and stdin contain that encoding, not raw value bytes.

SQL results and KV scans use JSON. SQL value tags preserve type distinctions; integers use decimal strings, blobs use base64, and floats use exact 64-bit hexadecimal representations, including numbers nested inside JSON values. Optional `--params-file PATH` uses the same lossless value encoding with the existing positional/named parameter containers. KV get emits raw bytes without an added newline. Successful set/delete commands produce no stdout and exit 0. Scans emit an ordered JSON array of key/value entries, with base64 values. Exit codes are 0 for success, 1 for query failure or a missing KV get, and 2 for invalid command usage; errors go to stderr. Empty values and empty scans are successful. CLI request timeouts are optional positive integer seconds, with no fixed default. Library deadline configuration uses `Duration`. One timeout budget starts after input is read and covers connection plus query completion. Timeout is not proof of rollback or a guarantee of hard interruption of blocking storage work.

### Build Roles

Default features are empty. Remote, embedded storage, hosting, and sync dependencies are enabled explicitly. A remote-only build must not include the local engine, Redb, Automerge, or sync implementation. Each query operation must document its atomicity. Clients do not automatically retry requests. Library request deadlines are caller-configurable, with no fixed default. Embedded and remote construction is explicit, not selected through a combined URI constructor. SQL query interfaces support typed statements and SQL-text helpers; remote SQL starts with existing statement execution.

The separate facades, embedded and remote query clients, host entry points, and one-shot CLIs are implemented with settled contracts and focused validation. KV errors preserve store categories through facade-owned structured errors. Internal path dependencies have registry version requirements, and publishable packages have descriptions. Registry-backed package verification and external-consumer checks remain blocked until prerequisites are available from a registry; no packages have been published.

### Engine Transaction

One transaction owns one complete read snapshot or write set for an engine operation batch. A write transaction commits all enlisted catalog, schema, index, Automerge row, and tombstone changes together, or rolls them all back.

### Logical Row

A Logical Row is the row value the Engine reads, writes, and indexes. Its stored representation is selected by the Row Reconciler.

### Row Reconciler

A Row Reconciler is the Engine's transaction-bound capability for storing, resolving, inspecting and transferring Logical Row state, metadata and document history. It owns no peer or wire-protocol state; engine-managed changes remain subject to the Engine's validated apply semantics.

### Row Identity

Every SQL Logical Row has one immutable internal UUID row identity, distinct from its user primary key. A user primary key is a non-null, unique tuple of one or more supported column values; a single UUID primary key also supplies the row identity, while natural and composite primary keys have independently generated UUIDv7 row identities. Use a single `UUID PRIMARY KEY` column for new tables: it supplies the row identity directly, auto-generates UUIDv7 when omitted on insert, and avoids a separate implicit key index. Natural and composite keys remain supported for cases where the key carries domain meaning. Caller-supplied row identities are RFC-variant UUIDv5 stable identities or UUIDv7 identities; generated identities and schema-definition identities are UUIDv7. The table name and that exact row UUID identify the row for storage and replication; independently created user rows with the same table and row UUID share one logical identity. A UUIDv5 identity does not encode creation time or establish authorization. A table's identity is its name, not its schema-definition row UUID.

### Schema Definition

A Schema Definition describes a named table or index and its fields. Competing table definitions select one complete schema by largest row UUID; deleting a losing definition does not drop the named table.

### Schema Compatibility

Compatible table schemas have equal field names, types, defaults, primary key, and unique constraints. Field order may differ, but index field order may not.

### Table Drop

A Table Drop deletes a named table's contents and dependent schema definitions, including stale writes made without observing its deletion. It differs from deletion of a losing schema-definition contender.

### Table Recreation

Table Recreation creates a new schema-definition row for the same table name after observing all relevant drops and starts with empty contents. It does not restore deleted rows or introduce a new logical table identity.

### Index Record

An Index Record is a derived mapping from an index key to a row UUID. For a unique key, the largest row UUID is the canonical contender, including when that contender is deleted.

### Conflict

A Conflict retains concurrent values within one Logical Row, with deterministic canonical ordering selecting the visible value. An explicit Resolution settles this conflict; collisions between distinct rows follow the separate Unique-Key Conflict rule.

### Unique-Key Conflict

A Unique-Key Conflict occurs when distinct Logical Rows claim the same unique-key value. The largest UUID wins, including tombstoned contenders; smaller contenders are permanently deleted rather than retained for promotion. This UUID ordering is deterministic, not a creation-time ordering for stable UUIDv5 identities.

### Generation

SQL table names identify tables. SQL table, column, index, and index-field rows have independent UUID primary keys; ownership and references are stored as ordinary values. SQL does not use table generations. KV generations remain a separate KV Store concept.

### Sync State Unit

A Sync State Unit is sync-owned canonical transferable state for one Logical Row, Table, Column, Index, or Index Field. It contains its identity, state bytes, deletion metadata where applicable, and a digest. Sync asks the Engine to apply it atomically while the Engine maintains derived Index Records.

### Sync Manifest

A Sync Manifest maps each compared SQL row identity to a digest of its document history and metadata. It describes the current comparison set, not a durable frontier or a whole-database snapshot.

### Committed Invalidation

A Committed Invalidation identifies replicated state that may need comparison after a successful commit, or requires a broader comparison when exact selection is unavailable. It contains identities rather than document payloads and is not an authorization scope.

### Selected Work

Selected Work is a consumer-specific invalidation set and completion watermark captured before reconciliation reads. Successful acknowledgement completes only that selection; later commits and other consumers remain independent.

### Document History

Document History is the native changes and dependencies that describe a document, including history retained inside snapshots. Native change identities are distinct from physical storage-record labels and from a digest of document heads.

### Tombstone

A SQL Tombstone permanently marks one Logical Row as deleted. Its unique-key claim can be superseded only by a different row with the same unique-key value and a larger UUID; this does not restore the deleted row.

### Application Schema Migration

An Application Schema Migration is an explicitly requested, ordered SQL change with an immutable version and completion history. It changes application schema or data, not the physical format covered by SQL Storage Compatibility.

### SQL Storage Compatibility

UUID row IDs replace the previous tagged and composite SQL row identities. Persisted SQL data and replicated payloads that use the previous identities are incompatible; do not mix formats or migrate them implicitly.

### KV Store

A KV Store holds shared `ofdb-value::Value` values under UTF-8 keys, with Automerge-backed history and tombstones. Values include primitives, blobs, and nested JSON objects and arrays. Concurrent same-generation changes merge through Automerge; tombstones win delete/update races. The greatest UUIDv7 generation determines visible state. This is not a last-writer-wins store.

### KV Expiry

KV Expiry is an optional absolute Unix-millisecond deadline. A value is hidden when the read time reaches that deadline. Expiry does not tombstone the generation or produce a committed invalidation; it is not a relative TTL.

### KV Key

A KV Key is a UTF-8 label for a value. It may have multiple KV Generations; the generation with the greatest UUIDv7 determines the visible state.

### KV Generation

A KV Generation is an immutable UUIDv7 identity for one KV Key's Automerge value. A tombstone ends that generation; setting the same key again creates a new generation rather than restoring the old one.
