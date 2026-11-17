# Spec: keep the redb shared host internal to the engine crates

Status: ready-for-agent

## Problem Statement

The redb shared-host machinery (one physical redb file partitioned into
namespaces, with a durable retirement fence and an optimistic multi-table
shared write transaction) is currently reachable from the published facades.
Both `ofdb-sql` and `ofdb-kv` re-export the redb-specific `SharedDatabase`
type and expose a `Database::open_shared(host, name)` constructor.

This breaks the documented facade contract:

- `Database` is a storage-agnostic control handle (`open`, `open_existing`,
  `in_memory`, hosting, sync). `Client`, from `Database::client()`, is the
  only query handle. Facades also support in-memory, postgres, remote, and
  IndexedDB, so a redb-specific type in the public API leaks the storage
  backend.
- SQL and KV are separate stores with separate clients. A shared host type
  exported from both facades invites physically co-locating SQL catalogs,
  KV stores, filesystem content, and private keys in one file: one corruption
  domain, and one shared write-admission lock that serializes every
  namespace against each other.

The app (`of`) local unified mode legitimately composes several owners into
one `unified.redb` file, but that is an internal server concern. Consumers
of the published crates should only ever do
`Database::open(path)` / `in_memory()` -> `client()` -> query/execute.

## Solution

Make the shared host internal-only:

1. Remove the `SharedDatabase` re-export and `Database::open_shared` from
   the public facades `ofdb-sql` and `ofdb-kv`. The facade public API stays:
   control handle `Database` + query handle `Client`, storage-agnostic.
2. Keep the shared-host machinery (namespaced tables, retirement fence,
   version-checked shared write transaction) in the internal crates
   (`ofdb-btree-redb`, `ofdb-sql-engine-redb`, and the SQL history shared
   stage), where it remains available to internal consumers.
3. The `of` app workspace already path-depends on the internal crates, so its
   local unified mode and its filesystem / IdP private-key consumers switch
   from reaching the shared host through the facades to using the internal
   crates directly.
4. Apply the same change to both mirrored crate trees (`of/db/*` and
   `ofdb/crates/*`); they are the same packages and must stay in sync.

## User Stories

1. As an external consumer of the published `ofdb-sql` crate, I want the
   public API to contain only the storage-agnostic `Database` control handle
   and `Client` query handle, so that my code compiles against in-memory,
   redb, postgres, remote, or IndexedDB without knowing which backend is in
   use.
2. As an external consumer of the published `ofdb-kv` crate, I want to open
   a KV store with `Database::open(path)` or `in_memory()` and query through
   `client()`, so that I never need a redb-specific host type.
3. As an external consumer, I want no `SharedDatabase` type or
   `open_shared` constructor in `ofdb-sql` / `ofdb-kv` docs, so that the
   shared-host concept stays an internal detail I cannot accidentally
   depend on.
4. As an `of` app developer running the local unified server, I want all
   owner and resource state in one `unified.redb` host with per-namespace
   isolation, so that loopback development keeps one file to back up.
5. As an `of` app developer, I want the unified composition to be built from
   internal engine crates only, so that the published facades stay clean
   while local mode keeps working.
6. As a storage-service operator, I want retiring a resource in unified mode
   to remove only that resource's namespace and leave a durable fence in the
   host file, so that a retired resource can never be reopened after restart
   and other namespaces stay usable.
7. As a filesystem runtime user in unified mode, I want filesystem root,
   metadata, content, and catalog KV stores to live as isolated namespaces in
   the shared host, so that one filesystem's retirement never touches
   another's data.
8. As an IdP service operator, I want the private-key KV namespace to stay
   isolated from every SQL catalog and resource KV store in the shared host,
   so that no query path can cross from a resource into private keys.
9. As a maintainer, I want a guard that fails CI if a redb shared-host type
   or constructor reappears in the facade public API, so that the boundary
   cannot silently regress.
10. As a maintainer of the two mirrored crate trees, I want both workspaces
    to build and test after the change, so that `of` and `ofdb` copies of the
    facades and engine crates do not diverge.

## Implementation Decisions

- Facade contract is fixed by the domain docs: `Database` is the control
  handle (storage lifecycle, hosting, sync) and `Client` from
  `Database::client()` is the only query/transaction handle. No ordinary
  query methods and no storage-backend-specific types on `Database`.
- The `redb` feature of each facade still provides ordinary file persistence
  via `open` / `open_existing` / `in_memory`; only the shared-host exports
  are removed.
- Shared-host types (`SharedDatabase`, `SharedNamespace`) and the
  shared write transaction stay public inside the internal engine crates
  (`ofdb-btree-redb`, `ofdb-sql-engine-redb`, `ofdb-sql-history`) because
  internal crates and the `of` workspace need them across crate boundaries.
- `Database::open_shared` is deleted from both facades. Where the `of` app
  still needs a facade-level `Database` over a shared namespace (control
  plane, migrations, `client()`), the construction moves into the `of`
  workspace's own adapter layer built on the internal crates; the published
  facade exposes no such path. The exact seam (internal feature on the
  vendored copy vs. `of`-local handle) is left to the implementer, but the
  published `ofdb` tree must contain no shared-host API.
- Internal consumers in `of` that currently import the shared host through
  the KV facade (filesystem runtime, IdP private-key repository) switch to
  importing it from the internal `ofdb-btree-redb` crate, which they can
  already path-depend on.
- Retirement semantics are unchanged: retiring a namespace drops only its
  tables and writes a durable fence; the host file is never deleted.
- Namespace isolation is unchanged: table-name prefixes per namespace, and
  open/check reject retired namespaces.
- The change is mirrored across both crate trees (`of/db/*` and
  `ofdb/crates/*`); the mirror sync is part of this work, not a follow-up.
- No changes to sync protocols, remote/hosting APIs, postgres, or the
  browser/IndexedDB paths.

## Testing Decisions

- Highest seam: the facade public API surface. Add a guard (CI script or
  API-surface check) asserting that `ofdb_sql` and `ofdb_kv` no longer export
  `SharedDatabase` or `open_shared`, and that `Database` exposes only the
  control-handle methods. External behavior only: nothing else about the
  facades may change.
- Behavior seam: the existing shared-host integration tests (KV shared,
  SQL shared, filesystem shared, sync-stage shared) must keep passing
  unchanged in intent, now driving the internal crates directly. These prove
  the internal path still delivers isolation, durability across restart,
  and retirement fences.
- Facade behavior seam: existing facade tests for `Database::open` /
  `in_memory` + `client()` queries, transactions, commit, and rollback must
  pass unchanged for redb and in-memory backends.
- Retirement seam: after retiring a namespace and restarting, reopening that
  namespace fails while sibling namespaces and the host file remain usable.
- Both workspaces' test suites run as final validation after the mirrored
  change.

## Out of Scope

- Deleting the shared-host machinery itself; it stays for internal unified
  local mode.
- Changing sync protocols, remote clients, hosting, or the CLI contracts.
- Browser/IndexedDB storage and Wasm builds.
- Postgres backend behavior.
- Migrating or re-laying-out existing `unified.redb` data files.
- Doc rewrites: the glossary and design docs already state the facade
  contract; only wording that implies facade-level `open_shared` may be
  touched.

## Further Notes

- The `of` and `ofdb` repos each contain a copy of the facade and engine
  crates (`of/db/*` mirrors `ofdb/crates/*`, same package names). Any change
  here must be applied to both trees; divergence is the main regression risk.
- `of/docs/backend-deployments.md` documents local unified mode as one shared
  host with per-namespace isolation and durable retirement fences; that
  behavior is preserved, only its reachability changes (internal crates, not
  facades).
- The `of` consumers that need switching are the filesystem runtime
  (`open_shared` for catalog/root/metadata/content KV namespaces) and the
  IdP private-key repository; the `of` db adapter and server crates already
  use the internal `ofdb-btree-redb` crate directly and mostly need only the
  SQL facade's `open_shared` replacement.
