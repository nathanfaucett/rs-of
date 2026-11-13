# Local persistence follow-up

Branch: `backend-simplification-persistence`, based on `f9209c816`.
Worktree: `.scratch/worktrees/persistence`. No merge back.

Status: **minimum startup/key requirements implemented; full single-file resource persistence incomplete**.
The spec is unchanged. Independent deployment constructors and layout are unchanged.
`storage-server/src/sync.rs` and sibling repositories are unchanged.

## Changes

- Fresh local startup creates only `unified.redb`, plus `tls/` when HTTPS is enabled. Owner resource roots are opened lazily, not created during startup.
- Local IdP private keys use the existing ofdb SQL client and native extended-private-key representation in a local-only `local_private_keys` table. No directory archives, resource snapshots, or custom key serialization were added. Independent IdP still uses its existing keyring implementation.
- All local IdP key purposes use this repository: Node signing and the User/Application/Client key service. The namespace is independent of the data directory path.
- Local Storage's retained public peer label uses a local-only SQL table rather than `storage-local.peer`.
- Local database files are created with mode `0600` on Unix. Existing files with group/other access, symlinks, and non-regular paths are rejected without changing their permissions. TLS files retain their existing handling.
- The owner database is not used as a resource SQL database. Process tests check that resource SQL cannot select the private-key table.

Private keys are plaintext at rest, as the former private files were. File access control is the protection; this is not encrypted storage. Local mode has no mesh synchronization. The local private-key table must never enter replication or be exposed to resource SQL. SQL history can retain old secret values; delete removes logical access, not historical bytes. A KV-based private store remains preferable once shared-handle APIs exist.

## Concrete persistence assessment

| State | Restart durability | In `unified.redb`? |
| --- | --- | --- |
| IdP users, credentials, public keys, clients, grants, permissions and signer metadata | Existing owner SQL persistence | Yes |
| Device identities, enrollments, selections and policy; Management selections | Existing owner SQL persistence | Yes |
| Local IdP private keys | New local-only SQL records; key repository and process restart tests pass | Yes |
| Local Storage peer label | New retained SQL record | Yes |
| Database identities, names, creation intent, namespace ownership and tombstones | Existing shared Storage SQL catalog | Yes |
| Resource SQL schemas, rows and history | Existing dedicated redb files; HTTP pre-shutdown row is readable after restart | **No** |
| Resource KV values and history | Existing KV sidecar files; existing restart test passes in Storage suite | **No** |
| Filesystem identities, names and tombstones | Existing ofdb KV filesystem catalogs | **No** |
| Filesystem metadata, contents, revisions and residency | Existing managed filesystem stores; new lazy-runtime content/tombstone restart test passes | **No** |
| SQL initialization, copy generations, pending cleanup, retired owners, passthrough selection | Existing durable copy-policy journals | **No** |
| Transport sessions, open handles, runtime caches | Intentionally process-local; reopen from durable state | Not applicable |
| Sync staging | Separate existing path; local sync handling belongs to the other agent | **No** |
| TLS certificate and key | Existing retained PEM files | Allowed `tls/` exception |

After resources are used, **back up the entire data directory**. `unified.redb` alone is not a complete resource backup.

## Why full resource consolidation is not implemented

`of-model::db::Database` exposes a query client and transactions, not a shared native store factory. The current public ofdb SQL `Database::open/open_existing` APIs own dedicated files. KV `Database::open_with_table` accepts a path and opens its own redb owner; it does not accept an already-open shared handle. Opening a second owner on the live unified file is not a supported solution.

The lower redb kernel has shared database machinery, but the application-facing SQL/KV owners do not expose it. Their retirement APIs also remove dedicated files and retain file-level fences. They cannot safely retire one resource inside `unified.redb` without new namespace-level lifecycle semantics.

`FileSystemCatalog::open` opens its own KV catalog and its resources use native roots. Metadata, staged content, revision publication, leases, retirement markers and removal are tied to those roots. There is no public constructor for a shared KV catalog plus a durable database content store. Archiving those roots into a blob would bypass the existing store/lifecycle APIs and was not done.

The remaining design must expose shared-handle, namespaced SQL/KV owners in ofdb, including resource-scoped retirement that cannot delete the host database. Offs then needs catalog/content/root construction over those stores. Copy-policy and namespace discovery must move into durable records as part of that change; directory scanning cannot remain authoritative. Existing KV namespaces and catalogs should be reused rather than converted to custom archives. These sibling library APIs were not changed on this branch.

## Complexity, risk and priority

The minimum change has moderate risk: it moves secret ownership and changes when resource roots are created. Tests cover private-file rejection, key load/delete/replacement across reopen, local constructors, HTTP isolation, filesystem content/tombstones, and process restart. Full resource consolidation has high risk because it changes retirement, cancellation, staged publication, and issued-handle fencing. It remains required for the full single-file specification; fresh-start layout is not proof of completion.

## TDD and validation

- The new private-file test failed first: the original database allowed group/other permissions. It passes after private creation and retained-file checks.
- The replacement key-repository test failed first against the old path-based constructor. It then exposed strict SQL parameter counting; the fixed transaction uses separate lookup parameters. Reopen, namespace separation, deletion, retained deletion and replacement pass.
- The fresh-layout process assertion failed with an extra `idp/` directory. Passing the shared root to local IdP removed it. Resource restart and filesystem tests are additional behavior coverage, not claimed as separate red/green fixes.

Final focused results:

| Command | Result |
| --- | --- |
| `cargo test -p embedded-db --test local` | 4 passed |
| `cargo test -p idp-service --features replica --test local_private_keys` | 1 passed |
| `cargo test -p storage-server --features network --test local` | 1 passed |
| `cargo test -p idp-server --test local` | 2 passed |
| `cargo test -p storage-service --lib local_runtime_defers_files_and_retains_resource_content_and_tombstones` | 1 passed |
| `cargo test -p unified-server --features cli --test composition -- --nocapture --test-threads=1` | 3 passed |
| Clippy on embedded-db, idp-service, storage-service, storage-server and unified-server, with replica/network/cli, `--lib` | Passed, with repository warnings |
| Rustfmt on changed Rust files; `git diff --check` | Passed |

The final composition run measured fresh HTTP 1.842 s, HTTP restart 0.491 s, fresh HTTPS 1.715 s, HTTPS restart 0.493 s, and dedicated startup 1.690 s. It checks exact fresh disk layout, Unix file permissions, unchanged JWKS, actual login/token exchanges, retained TLS, resource isolation, and a SQL row from the previous process. An earlier combined run exceeded the unchanged three-second HTTPS deadline; a subsequent individual run and the final complete run passed. No deadline was relaxed.

Broader checks are not green:

- `cargo test -p storage-service --lib`: **27 passed, 1 failed**. `deselection_retires_projected_and_created_copies_without_tombstones` reports that an issued domain session remains usable after retirement. This path was not repaired. A base-checkout reproduction was blocked by its missing `ofdb-buildfix` path alias, so no baseline pass/fail claim is made.
- Clippy `--all-targets`: blocked by an unchanged `async fn delete_user_by_id(&self, ...)` outside an impl in the test module at `idp-service/src/replica/user_repo.rs:1200`. Library-only Clippy passed.
- `cargo hack test --feature-powerset --workspace --all-targets`: two bounded 200-second attempts timed out; the first waited on a build lock, the second compiled feature combinations. No complete powerset result.
- `just crap`: bounded at 200 seconds and timed out during coverage dependency compilation. No CRAP score.
