# 03-remote-identity-database.md

Status: resolved

## Problem Statement

While the Unified Host uses in-process handles, we must ensure the architecture works correctly when services are deployed as separate processes. This requires verifying that the remote `ofdb-sql` g-RPC client correctly implements the same service surface as the local `Database` handle.

## Solution

Provide a path for IdP and Management to operate against a remote `ofdb-sql` g-RPC server. This involves configuring the services to use a remote client instead of opening a local file.

## User Stories

1. As an IdP operator running without the Unified Host, I want to point IdP at an ofdb-sql g-RPC identity database, so that IdP does not open a local identity file.
2. As a Management operator running without the Unified Host, I want Management to use that same remote identity database, so that roles and permissions stay consistent with IdP.
3. As a developer, I want the remote ofdb-sql client to satisfy the same query surface the in-process handle does, so that server code does not branch on where the database lives except at process startup.

## Implementation Decisions

- Use `ofdb-sql` g-RPC server to front the identity database.
- Configure services to switch between `Arc<Database>` (local) and `ofdb-sql` client (remote) based on the presence of a remote connection string.

## Testing Decisions

- **Multi-Process Test:** Start an `ofdb-sql` g-RPC server. Start IdP and Management as two independent processes, both configured to use the remote g-RPC server. Verify that a change made in one is visible in the other.

## Out of Scope

- Using a different database engine.

## Answer

Implemented on `implement/shared-control-plane-03`, based on integration commit `9e7360378b36cdd0385d6559afa53ae74034c005`. The current integration tip was merged into this worktree before reporting; it was already an ancestor. The original `remote-notes.md` was read and left unchanged.

### Implementation

- Identity repositories, conflict-clearance helpers, IdP query state, and `LocalPermissions` now take the existing `db::Client` facade. The same repository implementations use embedded or remote queries; no replacement repository family, query trait, local/remote enum, or request-path database opening was added.
- Local owners retain `Arc<Database>` for migration, synchronization and lifecycle. Query-only remote provisioners cannot manufacture those capabilities: `node_join_database` returns an error without a local owner.
- Added validated `identity_database` configuration to standalone IdP and Management. Connections have finite connect/query deadlines; invalid endpoints, unavailable owners, incomplete schemas, mixed credential history, missing conflict capabilities and conflicting policy/owner modes fail startup without a local identity fallback. Nonlocal SQL requires mTLS; plaintext is restricted to literal loopback IPs.
- Added the `identity-database` binary under `idp-server`: application-owned identity/policy migrations precede the existing ofdb-sql server. Remote HTTP consumers validate readiness, not migrate. The generic ofdb CLI remains application-agnostic.
- IdP private signer material remains in its local OS credential store. Remote reset uses local inventory and query-only metadata verification, never shared-database retirement/removal. Existing Installation attachment does not start the independent Create/Join wizard; direct remote wizard entry is explicitly rejected before provisioning. Remote peer-history import remains an owner capability, not a SQL query operation.
- Required companion work was committed separately in the original `ofdb` repository, initially clean on `main`: branch `integration/shared-control-plane-databases`, commit **`59ac5a5` — Add remote SQL conflict inspection and resolution parity**. Existing protocol/client/server/facade now transport conflict names, ordered batches, concurrent values, explicit partial resolution and transaction-scoped inspection. Transaction inspections use the same server-side transaction and see its uncommitted catalog; errors preserve abort/rollback behavior. Unimplemented executor capabilities still return Unsupported, not empty conflicts.

Risk is concentrated in security-history and transaction semantics; mechanical constructor changes are lower risk. No authentication/authorization conflict checks were bypassed or moved outside their transactions.

### Focused validation

All Cargo invocations used literal `CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target`. Worktree sibling dependency symlinks were inspected; the worktree uses the original sibling `ofdb`, not a duplicate checkout.

- TDD red: `ofdb` remote inspection failed with `QueryErrorKind::Unsupported`; configuration validation also failed before implementation.
- **External processes passed**, latest observed run **84.00s**: `permission_http_tests::remote_identity_process_tests::independent_processes_share_remote_identity_and_consumer_reset_preserves_rows`. A SQL-owner child, configured IdP child and configured Management child use production startup/query/router functions in separate OS processes. Authenticated HTTP proves deny → Management role/permission assignment → IdP allow → stopped Management consumer local reset → live IdP still allows → Management restart/revocation → IdP denies. SQL-owner shutdown fails closed. No IdP private identity file is created. Compilation was run with `cargo test -p unified-server --lib independent_processes_share_remote_identity_and_consumer_reset_preserves_rows --no-run`, then the resulting test executable was run separately to keep build and acceptance bounds independent.
- **Unified in-process regression passed**, latest observed run **79.40s**: `shared_identity_management_writes_change_idp_authorization`; assignment, reopen and revocation remain visible with no SQL listener.
- **Startup no-fallback test passed**: `cargo test -p idp-server --lib remote_identity_startup_never_falls_back_to_a_private_database`.
- **All 11 db feature combinations passed**: `cargo hack test --feature-powerset --all-targets -p idp-db`.
- **12 SQL tests passed** in `ofdb`: `RUST_MIN_STACK=16777216 cargo test -p ofdb-sql --features remote,sql,redb,in-memory,server,sync --test remote --test conflicts`. Includes real divergent conflicts, partial resolution, transaction snapshot/abort behavior, local/native regressions and existing mTLS client-identity acceptance/rejection.
- **Combined-feature all-target checking passed**: `cargo check -p idp-db -p idp-model -p idp-service -p management-service -p idp-server -p unified-server --all-targets`. Standalone IdP/Management binaries, including `identity-database`, also passed `cargo check -p idp-server -p management-server --bins`.
- **Strict affected-library Clippy passed** with `--lib --no-deps -- -D warnings`, for the six affected application libraries and for `ofdb-sql-client`, `ofdb-sql-protocol`, `ofdb-sql-server`, `ofdb-sql` with the exercised SQL features.
- Changed Rust files formatted; `git diff --check` and `python3 scripts/check_server_dependencies.py` passed.

### Exact limitations

- Application acceptance uses test-executable child processes invoking production startup/router functions, mock OS keyring storage, isolated successful OS-trust tool stubs, loopback plaintext SQL and 16 MiB runtime stacks. It does not qualify full CLI/native-provider installation, application mTLS rollout, real OS trust modification, WAN/container deployment or other platforms.
- The broader remote SQL query test overflowed the default small stack; the 12-test result above uses the explicit 16 MiB stack setting. Default-stack production qualification is not claimed.
- Dependency-inclusive strict Clippy stops at the existing `ofdb-btree-redb/src/btree.rs:95` needless-question-mark lint. It was not changed. Management `--all-targets` still encounters the base's stale `router/routes/device_authority_tests.rs` call supplying six arguments to the seven-argument `build_router`; no unrelated Device test repair was included.
- Full workspace feature powersets and `just crap` were not run under the requested focused-validation scope. Initial bounded builds/one partial db powerset attempt timed out on compilation/shared-cache locks; later focused checks, all 11 db combinations and executable acceptance completed.
- Configured remote serving attaches to an initialized identity Installation. Remote independent Create/Join wizard and peer-history synchronization are not implemented. IdP reset with protected inventory currently requires the SQL owner available for metadata verification; otherwise reset retains evidence and refuses safely.
- Device-server extraction and remote Device operation remain tickets 02/04. Until ticket02 is integrated, Management's existing Device side still uses its separate local handle; no Device remote implementation was added.

### Merge-sensitive changes for ticket02

- `management-server/src/{runtime.rs,provisioning.rs,lib.rs,config/app_config.rs,cli/run.rs,startup.rs,independent_create.rs,independent_join.rs}`: identity argument becomes `db::Client`, configured startup selection/readiness, and remote owner/wizard guards. Keep ticket02's Device extraction; carry these identity changes onto its role/permission-only runtime.
- `management-server/src/router/routes/replication.rs` and Device-related test files: only required identity repository constructor arguments changed to `.client()`; Device route behavior and models were not changed.
- `unified-server/src/{runtime.rs,security_bootstrap.rs,security_sync.rs}`: mechanical `.client()` query injection and propagation of fallible local-owner access. Local owner handles and Device composition are retained. Other Unified/Storage changes are corresponding test/fixture constructor updates, not a Device design change.
- `idp-server/src/local_provisioning.rs`: propagates the local-owner accessor error. Keep local bootstrap ownership intact when merging ticket02.


### Review-fix addendum

The earlier reset preservation statement was incomplete: `delete_local_secrets` also deleted shared public key metadata. Removed public SQL deletion from that local cleanup boundary for all callers. The remote Create IdP reset regression compares every column of every shared key row before/after reset and proves another IdP consumer still authenticates/signs. Local and remote independent IdP both use injected identity permissions; obsolete Device-as-permission-evaluator wiring is removed. See [ticket 05](05-review-fixes.md) for exact validation, red/green evidence, deployment limits and remaining unqualified checks; this addendum supersedes conflicting completion claims above.
