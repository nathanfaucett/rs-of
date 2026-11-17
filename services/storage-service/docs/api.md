# Storage API

Current owner/database boundaries and Device routes: [shared control-plane databases](../../../docs/control-plane.md).

These are current owner contracts, reconciled with the Management Device cutover; this documentation change is not a new runtime test or full-Node acceptance. Unified composition uses locally hosted IdP/Management owner boundaries and persistent trusted state. HTTP introspection of the local IdP is not a required remote authority. See [security](security.md), [system API contracts](../../../docs/api.md) and [qualification limits](../../../docs/operations.md#qualification-boundary).

## Identity and ownership

The namespace is `(subject, application_id)`. Derive both from validated IdP identity; callers cannot choose their namespace. Multiple OAuth Clients of one Application share that User's resources. Other subjects and Applications cannot discover or open them.

Each database or filesystem has its own stable, kind-specific ID and an optional, non-unique display name. A Filesystem ID is not a Database ID. There are no file permission bits, groups, cross-user grants, or per-folder ACLs.

Use IdP-issued Storage-audience access tokens from sign-in or exchange. Check the namespace and required `read` or `write` action on each request. These tokens do not authorize mesh synchronization.

## Storage routes

Routes below show the unified `/storage` prefix; standalone prefixes are configurable.

| Operation            | Route                                                | Action  | Success |
| -------------------- | ---------------------------------------------------- | ------- | ------- |
| Create database      | `POST /storage/databases`                            | `write` | `201`   |
| Ensure logical database | `POST /storage/databases/ensure`                     | `write` | `200`   |
| Execute atomic SQL batch | `POST /storage/databases/{database_id}/sql`           | parsed `read`/`write` | `200` |
| List databases       | `GET /storage/databases`                             | `read`  | `200`   |
| Get database         | `GET /storage/databases/{database_id}`               | `read`  | `200`   |
| Delete database      | `DELETE /storage/databases/{database_id}`            | `write` | `204`   |
| Create filesystem    | `POST /storage/filesystems`                          | `write` | `201`   |
| List filesystems     | `GET /storage/filesystems`                           | `read`  | `200`   |
| Get filesystem       | `GET /storage/filesystems/{filesystem_id}`           | `read`  | `200`   |
| Delete filesystem    | `DELETE /storage/filesystems/{filesystem_id}`        | `write` | `204`   |
| Read local Residency | `GET /storage/filesystems/{filesystem_id}/residency` | `read`  | `200`   |
| Set local Residency  | `PUT /storage/filesystems/{filesystem_id}/residency` | `write` | `204`   |

Create accepts an optional `name`. List returns only the caller's namespace. Unknown, deleted, wrong-kind, and wrong-namespace resources do not grant access; a delete that finds no matching resource returns `404`.

Deletion tombstones the catalog and removes cached runtime references. It does not invalidate existing in-process handles or guarantee immediate removal of all physical or offline copies. Deselection is a separate policy operation. Passthrough filesystems retain metadata without local file content and do not permit writes.

## Local whole-filesystem Residency

GET returns `{ "mode": "full" }` or `{ "mode": "passthrough" }`. PUT accepts exactly that body; unknown modes/fields (including paths) reject. These are filesystem-only operations on this Node, not the Device selected in Management, database modes, or synchronized policy.

Existing Storage authorization requires a current User access token with `storage:filesystems` audience/resource and the exact typed action (`read` for GET, `write` for PUT). The verified subject/Application namespace and exact local catalog filesystem ID determine the target; clients cannot provide an owner or Application override. Unified hosting additionally checks current local Device owner/policy and pending transfer cleanup through its existing owner callback. Unknown/wrong-namespace/deleted targets return404; fenced local copies return409. Neither setting Full nor reading a mode lifts cleanup fences, selects a resource, grants permission, or completes setup.

PUT uses `ScopedFileSystemRuntime::set_resource_residency_checked`: after asynchronous local opening, current actor authorization is checked again immediately before local policy publication, outside an owner write transaction. GET rechecks authorization and local-copy availability before returning its result. An admitted operation may finish; later operations evaluate learned denial. The existing trusted runtime setter delegates through the same implementation.

A204 means the whole-resource local policy and owner mode update succeeded, not that all content is available, a Full peer is reachable, or old cached bytes were erased. Failures do not claim success or silently reset policy/content. Publication failure before rename preserves prior durable state; failure/cancellation after publication may retain progress, so read/retry against current authorization. This is not a cross-owner crash-atomic rollback guarantee. Passthrough content uses the existing authorized peer path; no per-path durability or remote streaming endpoint is added.

The desktop uses generated `ResourcesResidencyApi` against the verified local `/storage` listener, with a transient explicitly supplied Storage User token cleared after each attempt. It does not reuse a Management token, redirect a resource denial into setup/reset, or present a mode acknowledgement as readiness.

## Management selection API

Management owns Device enrollment, endpoint bindings, ownership, approval/transfer/revocation, selections, restrictions and replication admission. IdP owns User/Client identity and token-signing trust. Services use authenticated owner APIs and separate repositories, including in unified hosting.

Routes below show the unified `/management` prefix. Management validates the User principal through its configured IdP owner relationship and checks Application-scoped `devices.select` in its own persistent store. Bundled composition uses direct current IdP bearer/active-User callbacks; independent composition uses authenticated HTTPS. Failure of the local identity/policy context denies; a remote peer need not be reachable. Standalone configuration must not be mistaken for completed equal-Node composition. See [system API boundaries](../../../docs/api.md).

| Operation         | Route                                                                                    | Current behavior                                                                 |
| ----------------- | ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Select resource   | `PUT /management/devices/{device_id}/selection`                                          | Validates Device and resource, stores selection, returns `204`                   |
| Deselect resource | `DELETE /management/devices/{device_id}/selection/{application_id}/{kind}/{resource_id}` | Checks current Management Device owner and permission; returns `204` or denial/not found |
| Deselect all      | `DELETE /management/devices/{device_id}/selection`                                       | Mounted but denies; use the resource-specific route                              |

Selection JSON contains `applicationId`, `kind` (`database` or `filesystem`), `id`, and `storageAccessToken`. The token is not supplied through an `X-Storage-Authorization` header. The Management token's Application must match `applicationId`. Resource-token audience is kind-specific (`storage:databases` or `storage:filesystems`), distinct from Management's configured Storage service-client audience.

Validate the supplied Storage token's signature, configured issuer, audience/resource, use, lifetime, subject, User principal, and typed Storage `read` action. The obsolete legacy `storage` scope is not required. Bind Device ownership to current Management records and check the kind-specific resource through the Node's Storage owner API. Unknown locally unverifiable resources and identity mismatch deny selection; offline locally verified resources remain usable. Deselect does not need content availability. Local owner HTTP lookup/introspection remains an authenticated service boundary, not a central authorization lease.

Selection starts empty and covers whole resources. Selection/deselection resolve the current Device through Management's own repository and active User evidence from IdP. Independent endpoint lookup uses Management `GET /devices/identities/{device_id}` with `management.device.lookup`, not an IdP Device route. Conflict-clear approval, canonical endpoint, active owner and exact policy are required, but an individual IdP token signer is not. A stored policy owner alone is not identity evidence; deselection needs neither content availability nor a Storage read token.

`GET /management/devices/{device_id}/restriction` returns `{ "adminAllowed": bool }`; `PUT` accepts exactly that typed body and returns `204`. They require separate Installation-scoped `management.device_restrictions.read` / `.update` grants with the exact Device target, and current Management Device binding. An Application grant cannot escalate; an administrator need not own the Device. Unknown/mismatched targets, malformed bodies and denied updates do not lift prior restrictions.

Management `PUT /devices/{device_id}/owner` accepts exactly `{ "ownerSubject": "User UUID" }`, requires the current approved owned Device, exact Installation `management.devices.transfer` and an active new User. Its204 means owner mutation, not cleanup completion. A learned mismatch records durable local cleanup intent and clears old selections/restricts allowance. Unified's native policy callback coordinates public SQL/KV and filesystem removal, revalidates Management ownership and rebinds an empty, restricted new-owner policy only after completion. Storage bearer/DATA gates deny until cleanup and explicit administrator allowance. Runtime owner admission remains fenced across restart; unrelated copies/catalog identities/Tombstones survive. See the [public owner contracts](../../../docs/public-persistence.md#managed-copy-retirement-public-owner-handoff). This is not setup/reset or Node-wide IdP/Management bearer gating. Learned restrictions apply to subsequent operations; admitted work may finish. No global offline blocking or remote erasure is promised.

`DatabaseRuntime::{remove_local_copy,reconcile_selected_copies,remove_owner_copies}` and `ScopedFileSystemRuntime::{remove_local_copy,remove_unselected_projected_resources,remove_owner_copies}` are async cleanup APIs. Completion means successful public owner retirement phases, not a replicated delete. Owner fences are permanent: a causally later observed selection uses a fresh copy generation under the same logical resource ID. Old handles and generation-pinned scoped sessions cannot follow it. Cleanup/reselection/retry intent is durable; stale unchanged selection cannot reactivate removed content.

`ScopedFileSystemRuntime::set_resource_residency` persists whole-resource Full/Passthrough. Scoped Passthrough `Read` uses a configured authorized reachable Full peer through existing DATA sessions, never cached content; write/delete/rename deny. `stream()` returns unavailable for Passthrough and owner per-path choices remain process-local. The reader returns bytes without materializing a local Full copy. Missing/untrusted/restricted/non-Full providers fail closed.

## Source

- Storage handlers: `crates/storage-server/src/router/resources.rs`.
- Storage authorization: `crates/storage-server/src/router/storage_authorization.rs`.
- SQL execution and wire contracts: `crates/storage-server/src/router/sql.rs`.
- Residency handlers: `crates/storage-server/src/router/residency.rs`.
- Selection handlers: `crates/management-server/src/router/routes/device_selection.rs`.
- Owner API checks: `crates/management-service/src/hosted_control_plane.rs`.

## Explicit application SQL contract

`POST /storage/databases/ensure` accepts exactly `{creationKey: string, name?: string}`
and requires database `write`. It explicitly creates or reopens; merely authenticating,
obtaining SDK Storage access, listing or getting metadata never provisions anything.
`creationKey` is an application-chosen, case-sensitive UTF-8 string of 1–128 bytes.
Names are non-unique labels; the first locally accepted name is retained on retry.

Identity is UUID v5 under UUID's URL namespace over UTF-8 JSON serialization of the
array `["of:database:creation:v1", verifiedSubject, canonicalApplicationId, creationKey]`.
The Application ID is lowercase hyphenated UUID, not Application URI, OAuth Client ID
or audience. No browser namespace override is accepted. The key identifies a logical
resource, not a short-lived HTTP request; separate tabs and lost-response retries use
the same key. Changing keys deliberately creates a different resource.

One Storage runtime owns a Node's catalog root. Creation is serialized with that
runtime's resource opening/deletion and durable catalog publication. An initializing
catalog record is synced before opening the SQL owner in a disposable staging
file. The closed initialized file is synced, renamed and its directory synced
before readiness publication. A stale unacknowledged staging file may be discarded;
published content is never discarded by ensure. This is catalog-linked initial
resource publication, not a second SQL engine/transaction or synchronization stage;
only the public SQL owner creates and validates the database. Readiness is synced before
acknowledgement. Retrying initializing intent completes initialization after restart.
Once ready, absent, corrupt or retired content is an error, never an empty replacement.
Ready-resource ensure and execution reopen through public `Database::open_existing`,
not the creating open; zero-length files are corruption, not a fresh database.
Internal lifecycle lookup includes initializing intents without exposing them through
public ready-resource lookup. Owner/local-copy retirement also fences these intents
and removes unacknowledged staging/published content; retries complete after restart.
Initializing intents cannot be opened through selected-copy provisioning.

Selected SQL opening is also existing-only for an acknowledged local copy, even when
protocol sync/replication reaches it before ensure or HTTP execution. Neither a
missing file nor a positive copy-generation number is evidence of fresh content.
Only explicit durable local `sql_initializing` intent permits staging a fresh SQL
copy: it is recorded before first selected catalog admission, or while completed
retirement is still fenced before advancing the copy generation. Current owner,
resource and local-copy fences must still allow admission. KV-first selected
admission records the SQL intent under the same SQL-open lock, so registration and
acknowledgement cannot race. An existing catalog entry without fresh intent always
requires existing valid content.

Fresh initialization publishes a closed/synced staging file before reopening it
through the public existing-only SQL owner. Existing published bytes are validated,
never reinitialized. Successful selected, ordinary or ensure SQL opens durably
consume local initialization intent **before caching/exposing the owner**. Interrupted
fresh provisioning can therefore resume after restart, but missing/zero-length
acknowledged content cannot become a cached empty owner, including after reselection.
A Tombstone permanently consumes that identity: ensure returns `410`; deliberate
recreation requires a new key. Local-copy fences return `409` and are not lifted.
Unknown metadata lookup remains `404`; acknowledged missing/corrupt content is `503`.

Disconnected Nodes with the same verified namespace and key derive the same ID and
may independently initialize copies. This is identity convergence, NOT global create
serialization, single-copy existence or globally ordered writes. Local labels may
differ until catalog reconciliation. Existing selected-resource synchronization and
CRDT conflict semantics apply; no Device selection, enrollment or synchronization
permission is implied. A Node unaware of a remote Tombstone may operate until it
learns deletion; learned Tombstones deny subsequent ensure/execution. No central
writer or live authority is introduced.

`POST /storage/databases/{database_id}/sql` accepts exactly
`{statements: [{sql: string, params?: SqlValue[]}]}`. One server-owned transaction
executes the complete batch in order, commits only after all statements and result
validation succeed, and rolls back on failure. No transaction handles or automatic
mutation replay. Parameters are positional (`?`/`$1` as supported by the owner).
Wire values are tagged `{type: "null"}`, or `{type, value}` for `boolean`, `integer`
(decimal i64 string), `float` (finite JSON number), `text`, `uuid` (UUID string),
`blob` (byte array). Owner Type/JSON values are unsupported (`unsupportedValue`);
applications can store serialized JSON in text. Results are
`{results: [{columns: string[], rows: SqlValue[][]}]}` in translated-statement order;
DDL/writes may return no rows. No affected-row count is promised.

Authorization classifies parsed owner operations. Any DDL or mutation requires
`write`; SELECT requires `read`. INSERT/UPDATE/DELETE with RETURNING require both
`read` and `write`, even for a no-op update. Mixed batches need all required actions.
Classification and supported-shape validation finish before any statement executes.
Authorization and namespace are rechecked immediately before transaction admission
(without holding an owner transaction across IdP/Device/network calls). Catalog
liveness and local-copy fences are checked after admission and before commit; an already admitted operation may finish. Response authorization is checked again after commit, outside the transaction;
denial then suppresses the response but cannot undo committed work. Tokens are sent only
in Authorization headers. SQL/parameters/results and owner error text are not logged
or returned in failures. Errors use `{code}` with redacted fixed codes: `invalidRequest`, `executionFailed`,
`unsupportedValue`, `unauthorized`, `forbidden`, `notFound`, `deleted`,
`localCopyFenced`, `limitExceeded`, and `unavailable`.

Bounds: 256 KiB HTTP body; 1–32 submitted and translated statements; 16 KiB SQL per
entry; 128 parameters per entry; at most 1,000 total result rows and 1 MiB serialized
results per batch. Oversized results fail the whole transaction, never truncate a
snapshot.

The HTTP SQL subset supports CREATE TABLE (including IF NOT EXISTS and inline
indexes/constraints), literal/parameterized INSERT VALUES (including supported
ON CONFLICT), and single-table SELECT/UPDATE/DELETE with scalar predicates.
SELECT may order, limit, or offset; projections/RETURNING contain at most 128
non-repeated column names (or a lone `*`). Predicates support scalar values,
comparisons, AND/OR/NOT, IS NULL/IS NOT NULL, and literal IN lists, with depth at
most 32. Joins, subqueries/EXISTS, aggregates, grouping/HAVING, DISTINCT, computed
projections/concatenation, LIKE, repeated projections, and other DDL are rejected
with `400 invalidRequest` before owner transaction admission. Unsupported SQL is
not translated into a silently truncated snapshot. This is a generic bounded CRUD
contract, not unrestricted owner SQL hosting.

Every HTTP transaction sets the public native owner budget to 65,536 read entries
and 8 MiB encoded key/value bytes, cumulatively including catalog/repeated reads.
Native admission checks borrowed bytes before owned decoding/range accumulation.
A separate 65,536-row/8 MiB decoded scalar-row budget covers scans, mutation matching,
and inserted/updated rows before accumulation/encoding. Even a plain SELECT, an
unmatched WHERE, or LIMIT 1 cannot bypass scan budgets. Exhaustion returns `413
limitExceeded` and aborts the batch. The stricter 1,000-row/1 MiB wire checks still
apply before commit. Budgets include storage/history/catalog overhead, so they
can reject a query whose projected response would otherwise fit.

This prevents unbounded scan retention and query-driven join/projection amplification;
it is not a hard process-memory, decompression, CPU, concurrent-request, or wall-clock
isolation guarantee. A codec decodes one row/document before its logical size is known;
native encoded reads are bounded first. Oversized replicated/history-heavy resources
may need trusted native/operator recovery rather than HTTP editing. Cancellation or
timeout after send does not prove rollback; callers must reconcile before retrying.

### Qualification boundary

The ticket-01 tests exercise real IdP sign-in/token exchange and authenticated
HTTP backed by native catalogs and the public SQL owner, including separate
User/Application namespaces, multi-Client continuity, generated Rust scalar
roundtrips, rollback, retry, local restart and fences. The subprocess crash test
interrupts a creation worker and reopens through the public owner; it does not
deterministically enumerate every power-loss cutpoint. Independent runtime tests
verify derived identity and locally learned Tombstone behavior, not deployed
mesh/catalog synchronization or CRDT conflict resolution. Loss of the entire
catalog/Node root is not distinguishable from a fresh Node; operator recovery
requires restoring trusted metadata/content, not invoking ensure as a repair tool.
SQL does not add encryption; use non-sensitive test records.
