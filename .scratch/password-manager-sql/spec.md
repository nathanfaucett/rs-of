# Password manager: application-provisioned SQL storage

Status: ready-for-agent

## Problem Statement

The password manager currently requires the User to discover, select, or manually create a Filesystem Storage Resource before managing secrets. It persists individual secrets as JSON files through a filesystem-backed TanStack collection. Filesystem concepts and resource identifiers are exposed as mandatory application setup even though the User only wants to manage passwords.

The platform already supports durable SQL Database Resources, but the browser-facing Storage interface and authenticated SDK currently expose filesystem operations rather than application SQL execution. Existing database creation generates fresh resource identities and permits duplicate display names; naive list-then-create provisioning could produce separate databases during concurrent startup or retries.

Users need a database that the application creates and reopens automatically, without manual database administration, weakened namespace isolation, or silent replacement of inaccessible data.

## Solution

After IdP authentication, the password-manager application resolves its default Database Resource, explicitly creates it when appropriate, initializes or checks its application-owned schema, and opens the secrets editor. Subsequent visits reopen the same logical database. The normal interface contains no filesystem/database selector, manual resource-creation step, or required database identifier.

The application owns default selection and provisioning policy. Storage provides generic authorized database lifecycle and SQL operations. The Storage client and SDK expose those operations without automatically provisioning resources. The existing reactive collection/editor behavior remains, with SQL replacing filesystem persistence.

Provisioning, loading, empty, ready, and failure states remain distinguishable. An unavailable, denied, deleted, or malformed existing database must not silently become a new empty vault.

## User Stories

1. As a User, I want to sign in through the IdP, so that my existing identity authorizes access to my password-manager data.
2. As a first-time User, I want the application to create its database automatically, so that I can start adding secrets without configuring Storage Resources.
3. As a returning User, I want the application to reopen the same logical database, so that my saved secrets remain available across visits.
4. As a User, I want to reload the browser and recover persisted records, so that persistence does not depend on an in-memory collection.
5. As a User, I want the application to hide database IDs and filesystem concepts during ordinary use, so that setup matches the task of managing secrets.
6. As a User, I want clear preparation and loading feedback, so that I know when my database and records are ready.
7. As a User, I want a newly initialized database to display an empty secrets list, so that I can distinguish a fresh vault from a loading failure.
8. As a User, I want provisioning failures to offer an explicit retry, so that transient failures do not require manual database creation.
9. As a User, I want retries after an uncertain creation response to resolve the intended database, so that a network interruption does not split my records across duplicate databases.
10. As a User, I want concurrent tabs opening the application to resolve the same intended database, so that simultaneous startup does not create competing defaults.
11. As a User, I want unavailable existing content to produce an error rather than an empty replacement, so that a temporary outage cannot hide my saved secrets.
12. As a User, I want deliberate database deletion to remain meaningful, so that automatic startup does not silently resurrect a deleted resource.
13. As a User, I want only my User/Application namespace to be accessible, so that another User or Application cannot discover or modify my secrets.
14. As a User, I want OAuth Clients of the same Application to share the intended namespace, so that changing a registered Client does not create a different ownership model.
15. As a User, I want to create, edit, and delete secrets through the reactive collection, so that list updates and persistence follow one consistent workflow.
16. As a User, I want favorite-first and name ordering to remain, so that switching persistence does not change how I find secrets.
17. As a User, I want edits to preserve additional websites, match modes, custom fields, notes, timestamps, and password history, so that editing a visible field does not discard other record data.
18. As a User, I want required fields validated before persistence, so that invalid drafts do not become saved records.
19. As a User, I want save/delete success reported only after persistence and reconciliation complete, so that optimistic updates are not mistaken for confirmed writes.
20. As a User, I want rejected writes to show an error and reconcile the list, so that I can review the authoritative state before retrying.
21. As a User, I want uncertain writes identified without automatic replay, so that a timeout does not cause duplicate or unintended mutations.
22. As a User, I want Refresh to reload external additions, edits, and deletions, so that I can explicitly obtain current records.
23. As a User, I want a successful Refresh to close stale unsaved edits, so that I do not unknowingly save a draft based on an older snapshot.
24. As a User, I want malformed or partially failed loads to hide untrusted records and show an error, so that incomplete data is not presented as a valid vault.
25. As a User, I want sign-out and authenticated identity changes to immediately remove records and drafts, so that old secrets cannot appear in a replacement session.
26. As a User, I want ordinary token renewal to retain my current editor, so that renewing the same identity does not discard my work.
27. As a User, I want Storage denial or connectivity failure distinguished from terminal IdP authentication failure, so that a database error does not unnecessarily sign me out.
28. As a User, I want clear disclosure that changing persistence does not add end-to-end encryption, so that I do not mistake this example for a qualified production password vault.
29. As a User with existing filesystem records, I want clear notice that those records are not automatically imported, so that an empty SQL database is not mistaken for data loss.
30. As an application developer, I want explicit generic database operations from the SDK, so that my application owns when and why resources are created.
31. As an application developer, I want the application to own schema initialization and version checks, so that generic clients do not contain password-manager policy.
32. As a Node operator, I want database operations to respect current local resource and Device restrictions, so that browser access cannot bypass Storage lifecycle or replication policy.
33. As a maintainer, I want the default database identity contract to account explicitly for disconnected Nodes, so that local concurrency protection is not misrepresented as global uniqueness.
34. As a maintainer, I want bounded, parameterized SQL requests and redacted failures, so that exposing SQL does not leak credentials or permit unbounded work.

## Implementation Decisions

### Ownership and module interfaces

- The password-manager application owns resolving its default database, deciding when to request creation, choosing any stable logical creation key, initializing/checking its schema, and opening the secrets collection.
- Keep this startup coordination in an application-owned module rather than scattering provisioning across page effects, editor logic, or collection persistence callbacks.
- Authentication restoration and obtaining a Storage capability must not create resources. Explicit database creation is a generic SDK operation called by application policy.
- The SDK composes authenticated clients and guards operations against session retirement. It does not own default databases, application schemas, vault naming, collection registries, or secret models.
- The Storage client owns token exchange and database transport contracts. The SDK preserves captured authentication-generation and cancellation guarantees around these operations.
- The collection adapter receives an already resolved database and a narrow structural Storage SQL interface. It owns record validation, SQL persistence mapping, optimistic settlement, reconciliation, Refresh, and cleanup. It does not select or provision the default database and must not depend on the concrete SDK implementation.
- Storage owns Resource Catalog identity/lifecycle, namespace isolation, authorization, and application-facing SQL execution. It must not gain a password-manager-specific endpoint or default-vault concept.
- Reuse the existing public SQL owner facade. SQL execution, transactions, persistence, and generic synchronization/integrity belong to `ofdb`; domain identity/catalog/authorization integration belongs to Storage in `of`.
- Device selection, restrictions, and replication policy remain separate. Database creation or app startup must not implicitly enroll Devices, select resources for replication, or expand permissions.

### Authentication and namespace

- Authenticate the User through the IdP using an OAuth Client belonging to the password-manager Application.
- Exchange the current verified User access token at the IdP for a Storage-Audience Access Token targeting `storage:databases`, with the actions required by the operation.
- Preserve the User subject and canonical Application association through token exchange. Storage derives the namespace from validated authorization, never trusted browser-supplied owner/Application overrides.
- Distinguish the logical Application URI, canonical internal Application ID, OAuth Client ID, and token audience. Multiple OAuth Clients of the same Application share the User/Application namespace.
- Extend the current filesystem-specific exchange implementation to select the correct resource audience while preserving existing filesystem behavior for other consumers.
- Use authorization headers for SQL HTTP requests; do not place tokens in query strings.
- Preserve same-identity renewal, immediate local session retirement, rejection of obsolete captured capabilities, cancellation of unsent work, and suppression of late results/notices. Local cancellation does not prove remote rollback.

### Database creation identity and lifecycle

- Use an automatically opened default Database Resource in the existing User/Application namespace, not a separate shared password-manager backend with custom row tenancy.
- Database display names remain non-unique labels, not logical identity. Do not implement default resolution as selecting by name or uncoordinated list-then-create.
- Provide the generic Storage identity/creation guarantee needed for application-owned provisioning. The application chooses the intended logical identity; Storage validates and enforces its generic namespace-scoped semantics; the SDK/client forwards the explicit request without inventing keys or policy.
- Request idempotency and stable logical identity are distinct requirements. Retrying one request and independently provisioning from multiple tabs must both resolve the intended resource without duplicate defaults on the same Node.
- Before implementing creation, specify the exact generic mechanism, durability, interrupted-create recovery, deletion/recreation semantics, and behavior on disconnected Nodes. Keyed creation and caller-supplied resource identity are candidates, not approved existing contracts.
- A process-local mutex or browser-local preference is not evidence of cross-Node uniqueness. Define how independent offline provisioning relates to the platform's equal-Node/resource identity model; do not introduce a central authority or designated writer.
- Preserve Tombstones and local-copy retirement fences. Distinguish absence from deletion, unavailable content, corruption, and authorization denial. Never automatically replace unavailable acknowledged data with an empty database.

### SQL transport and persistence

- Add generic authenticated application SQL execution to Storage; existing resource lifecycle routes alone are insufficient.
- Use one bounded HTTP SQL execution operation with parameterized statements and atomic batches executed within a server-owned transaction. Separate CRUD endpoints, long-lived browser transaction handles, and a new persistent WebSocket query protocol are unnecessary for this feature.
- Enforce current User/Application namespace, resource lifecycle/local-copy availability, and required actions at execution. A read-only token must not execute mutations, including DDL or mixed batches; classify parsed/translated operations rather than matching SQL string prefixes.
- Reuse existing SQL translation/execution capabilities rather than exposing raw database hosting, peer replication protocols, internal persistence crates, or server filesystem paths.
- Define wire value/result types, structured redacted errors, and appropriate request/statement/result bounds. Reuse existing owner facilities where available rather than duplicating generic engine mechanisms.
- Never log access tokens, secret parameters, result contents, or credential-bearing error bodies.
- The application owns repeatable schema initialization and schema-version checks. Initialize related schema/version state transactionally; incompatible or malformed state must fail clearly rather than be silently reset or migrated.
- Start with one secrets table containing a secret ID and its serialized validated record. Keep favorite/name ordering in the existing TanStack live query. Typed scalar columns, nested-table normalization, and SQL-side filtering/indexes are unnecessary for the current editor; add them only for demonstrated query needs.
- Validate all loaded records against the existing Secret schema before publishing a complete collection snapshot.

### Application cutover

- Replace the password manager's filesystem-backed persistence with SQL-backed collection persistence. Keep the official TanStack Svelte live-query binding and the collection mutation path; do not add a parallel secrets array or direct-write bypass. Keep the persistence implementation focused on this consumer rather than building a reusable SQL collection framework.
- Remove mandatory filesystem selection/open/create controls and obsolete app-specific filesystem wiring, fixtures, and documentation. Preserve generic filesystem capabilities used elsewhere.
- Preserve required-field validation, metadata/history preservation, favorite/name ordering, optimistic settlement, explicit Refresh, single-context cleanup, and conservative uncertain-write handling.
- Provisioning retry is explicit and safe under the generic creation contract. Secret mutations are not automatically replayed after uncertain outcomes.
- No implicit import, dual-write, fallback, or compatibility wrapper for existing filesystem records. Leave those resources untouched and document the cutover clearly.
- SQL persistence does not add encryption or automatically solve concurrent-client stale edits. Retain truthful security/concurrency limitations unless a separately agreed design expands the scope.

## Testing Decisions

- Test observable behavior across public interfaces, not internal helpers, private state, table layouts, catalog filenames, or serialization implementation details.
- The primary app-level seam is the existing mounted password-manager workflow using the real SDK, OIDC/Storage clients, collection implementation, and official TanStack Svelte binding with controlled transport. Reuse the current mounted workflow and redirect/callback test patterns rather than adding a testing-only abstraction.
- Cover fresh sign-in automatically provisioning and opening a ready empty database, returning visits/reloads reopening persisted data, repeatable schema initialization, provisioning errors/retry, CRUD, ordering, record preservation, Refresh, malformed snapshots, rejected writes, uncertain writes, and no false success.
- Cover immediate clearing on sign-out/identity changes or teardown during provisioning, load, Refresh, and writes; late results must not populate a replacement context. Same-identity renewal must retain the editor/collection.
- Extend existing Storage client and SDK lifecycle tests for correct database audience/actions, prefix composition, request authorization, cancellation before sending, stale capabilities, and obsolete completions. Verify authentication restore and obtaining Storage access have no provisioning side effects.
- The server-side seam is the authenticated Storage HTTP interface, backed by real catalog/runtime/SQL persistence where practical. Controlled browser responses cannot establish server authorization, durability, or transaction correctness.
- Verify User/Application isolation, same-Application multi-Client namespace continuity, wrong audience/actions, spoofed namespace attempts, read-only mutation denial, mixed-batch authorization, rollback on batch failure, and current deleted/fenced-resource denial.
- Verify concurrent provisioning, retry after an uncertain response, restart, interrupted creation/schema initialization, Tombstone behavior, and no empty replacement of unavailable content. Add disconnected-Node identity tests matching the contract chosen before implementation; do not claim global uniqueness from single-Node tests.
- Keep genuinely generic SQL transaction/integrity tests with the persistence owner. Add only demonstrated missing coverage there; domain token/catalog policy belongs in Storage integration tests.
- Use focused package/crate checks while implementing. Run applicable formatting, type checking, builds, tests, Rust feature-combination validation, and complexity checks for the implementation's changed scope.
- Perform real-system qualification with non-sensitive records: actual IdP sign-in/token exchange, automatic creation, SQL CRUD, browser reload, Node restart, denied access, and recovery. Qualify replication separately with authorized selected Nodes if claimed; fixtures or same-Node Refresh do not prove replication.

## Out of Scope

- Default database selection, automatic provisioning, or password-manager schema logic inside the SDK or generic Storage backend.
- A password-manager-specific backend or a shared cross-User SQL database with new row-level tenancy.
- Replacing the existing SQL engine with SQLite, PostgreSQL, or another persistence dependency.
- Rewriting all Resource Catalog persistence or unrelated host/owner state.
- Implicit filesystem migration/import, dual-write, fallback persistence, or deletion of old filesystem data.
- Multiple-vault management or an advanced database selector.
- Browser-offline mutation queues, persistent browser secret caches, automatic mutation retries, polling, automatic reconnect, or live cross-tab synchronization.
- Automatic Device selection/enrollment or changes to replication permissions.
- New centralized coordination or claims of globally serialized offline writes.
- End-to-end encryption, vault unlock/recovery/key-management implementation, secure erasure guarantees, or production password-manager qualification.
- Concurrent-edit conflict prevention beyond the existing documented behavior, unless separately agreed.

## Delivery Plan

Deliver two complete, independently verifiable slices:

1. Explicit authenticated SQL database access: generic creation identity/lifecycle, SQL execution, client contracts, and session-guarded SDK access, verified through authenticated requests and client/SDK tests. No application defaults or automatic creation in the SDK/backend.
2. Password-manager cutover, blocked by the first slice: application-owned automatic provisioning and schema initialization, SQL-backed load/save/delete/Refresh, removal of obsolete filesystem UI/wiring, recovery tests, documentation, and qualification together. Do not create separate read-only UI, Refresh, or cleanup milestones.

## Further Notes

The existing database engine and resource lifecycle operations reduce the amount of new infrastructure needed, but database creation is not currently a safe ensure-default primitive, and the application SQL HTTP interface does not yet exist. Do not treat candidate method names or creation-key mechanisms discussed during planning as implemented capabilities.

The generic resource-identity contract is the first implementation design task. Resolve and document it before implementing the browser startup flow; do not conceal unresolved disconnected-Node or deletion behavior behind a local list-then-create shortcut.

SQL stores secret values unless an encryption design changes the payload. The current example explicitly lacks client-side encryption. SQL/CRDT history and replicated copies can retain overwritten or deleted values. Use test secrets only; encryption/key ownership must be designed before presenting this as a real-password vault.

This spec records the agreed boundaries and testing approach. It does not claim implementation, newly passing tests, deployed IdP qualification, or cross-Node convergence evidence.
