# 04-remote-device-database.md

Status: resolved

Claimed by: `implement/shared-control-plane-04`, worktree `04`, base integration `ba45e15f5`; original sibling `ofdb` at `59ac5a5`.

## Implementation log

- Read original spec, ticket02/03 Answers and `remote-notes.md`; called `tdd`. User confirmed standalone startup/provisioning/reset and authenticated external Storage admission seams.
- Complexity/risk/priority: mechanically reuse `db::Client`; high-priority security boundaries are conflict/transaction parity, current approval, startup no-fallback, and reset ownership. No security checks or user-resource replication are removed.
- Startup TDD red: remote configuration was ignored and `open_device_database` created a private file. Green: explicit remote configuration/local-owner exclusion, with real remote startup validation.
- Admission TDD red: router required `Arc<Database>` and no remote startup entry point existed. Process acceptance now passes trusted allow, unknown/revoked deny, consumer reset/restart preservation, SQL-owner outage and Device outage through the existing authenticated HTTP adapter.
- Remote Create/Join/reopen passed against shared identity and Device SQL owners, including actual IdP login, authorized signed source approval, target reopen and live-source rows after target Device reset. The first fixtures needed valid OAuth issuer/UUID claims and the established `offer_id` field; one 30-second IdP readiness timeout was observed and retained as a validation limit.
- Reset TDD red: a completed remote consumer reset removed an unexpected `shared-owner.redb` fixture. Fix requires an empty, nonsymlink remote consumer data root before any reset actions; setup inventory, protected-secret and OS-trust checks remain.

Exact commands and final outcomes are recorded in the Answer below when complete.

## Problem Statement

Similar to the identity database, the Device database must also be accessible via g-RPC to support out-of-process operation of the Device server and to allow Storage to query it remotely.

## Solution

Expose the Device database via `ofdb-sql` g-RPC, allowing the `device-server` to run as a standalone process and allowing `storage-server` to query it remotely.

## User Stories

1. As a Device server operator, I want Device state served from its own ofdb-sql g-RPC database, so that Device lifecycle can run without the identity database process.
2. As a Storage operator, I want replication admission to ask the Device server, so that Storage does not open the Management or Identity databases.
3. As a Developer, I want the remote `ofdb-sql` client to satisfy the same query surface the in-process handle does, so that server code does not branch on where the database lives except at process startup.

## Implementation Decisions

- Use `ofdb-sql` g-RPC server to front the Device database.
- `storage-server` will be updated to use a remote `device-server` client for admission checks.

## Testing Decisions

- **Remote Admission Test:** Start a standalone `device-server` via g-RPC. Start a `storage-server` and verify that it can successfully admit a device reported by the remote `device-server`.

## Out of Scope

- Changing how user-data replication (the iroh sync path) works.

## Answer

Implemented in worktree `04` on `implement/shared-control-plane-04`, based exactly on integration `ba45e15f5`. Original spec, ticket02/03 Answers and original `remote-notes.md` were read. Original sibling `ofdb` remains unchanged at `59ac5a5` on its existing dependency integration branch.

### Delivery

- Added the domain-owned `device-database` binary: Device-only migrations, existing ofdb-sql gRPC serving, loopback-only plaintext or complete mTLS configuration. No generic SQL-owner application schema changes.
- Existing Device repositories, transactional conflict checks, selection convergence and router query state now use `db::Client`, locally injected or remote. Local owners retain migration/history/sync capabilities; no alternate repository family, query trait, local/remote enum, raw SQL parser or compatibility path was added.
- Device startup selects `device_database` and `identity_database` through the established remote adapter; validates schema, mixed enrollment history, conflict and transaction capabilities; rejects competing local-owner/remote settings and remote errors without private-file fallback. Explicit local identity ownership remains available; Unified injects clients without gRPC.
- Storage retains the existing authenticated Device HTTP client-credentials adapter. Separate SQL-owner, Device and Storage-probe processes prove externally observable trusted allow, unknown/revoked denial, consumer restart/reset preservation, SQL-owner outage and Device outage. The Storage child receives only HTTP credentials and no database handle/SQL configuration.
- Completed remote independent Create and Join across IdP/Device/Storage. Create provisions local private material and least-privilege relationships through existing workers; Device seeds identity grants through the identity service/query client instead of depending on `management-server`. Join requires the existing exact signed source approval, a current source-authority check and current approved Device/policy/signer rows in the same Installation's shared owners. It cannot import history into a different empty remote owner; history remains a local-owner operation. Completed remote consumers restore without replay or private database creation.
- Remote Device reset removes consumer setup/protected material only. A stateless attachment needs no SQL contact; a setup-managed remote root must be empty and nonsymlink before reset begins. Unexpected owner data is retained even with valid setup evidence. Membership and both shared databases are preserved; revocation is an explicit authorized API operation, not reset collateral damage.
- Fixed the merger's three required blockers: stale IdP bootstrap settings test now uses Device settings, removed `device-server -> management-server`, collapsed Unified's admission nested `if`. Mechanical constructor call-site updates preserve user replication and all conflict/cleanup/last-approved-Device safeguards.
- `docs/control-plane.md` documents actual owner commands, TLS/credential boundaries, startup configuration, fresh Create, signed Join, attachment, reset and qualification. Updated the remote identity operations section to supersede ticket03's wizard limitation. No frontend/UI or user-resource protocol changes.

### Exact validation commands and scope

All commands below ran from `/home/nathan/code/rust/aicacia/of/.scratch/shared-control-plane-databases/worktrees/04`. Every Cargo test/check/Clippy invocation used this literal environment prefix unless the prefix appears inline:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_INCREMENTAL=0 RUST_MIN_STACK=67108864
```

Use it as environment assignments before `cargo`, not as a separate shell command. Executable acceptance commands set their stack explicitly as shown. Builds and execution were bounded separately where practical.

Passed final focused commands:

```sh
cargo check -p device-server -p idp-server -p management-server -p unified-server --all-targets --message-format short
cargo test -p device-server -p management-server --no-default-features -- --test-threads=2
cargo test -p device-service --features replica --lib -- --test-threads=2
cargo test -p idp-server --no-default-features --test bootstrap_settings
cargo test -p storage-server --no-default-features --features network --lib device_client -- --test-threads=2
cargo test -p storage-server --no-default-features --features network --lib local_replication_authorizer -- --test-threads=2
cargo test -p unified-server --no-default-features --test composition
cargo clippy -p device-service -p device-server -p idp-server -p unified-server --no-default-features --lib --no-deps -- -D warnings
cargo test -p unified-server --no-default-features --lib --no-run
```

Device-service: 56 unit tests; Storage: 11 adapter and 4 authority tests; composition: 2 tests; bootstrap-settings: 1 test. Device/Management server suites and hosted-join tests passed. Initial Device-service compilation exposed legacy tests using the repository's former owner field; those fixtures now retain their owners separately for synchronization and the 56-test suite passes. All-target four-server checking includes both SQL-owner binaries and server tests. Strict Clippy is affected-library/no-deps scope, not workspace/dependency-inclusive qualification.

Final executable acceptance commands (after the build above):

```sh
RUST_MIN_STACK=67108864 /home/nathan/code/rust/aicacia/of/target/debug/deps/unified_server-3151668e4144c159 device_authority_tests --test-threads=1 --nocapture
RUST_MIN_STACK=67108864 /home/nathan/code/rust/aicacia/of/target/debug/deps/unified_server-3151668e4144c159 --exact owner_composition_tests::remote_setup::remote_create_join_reopen_and_device_reset_preserve_shared_installation --nocapture
RUST_MIN_STACK=67108864 /home/nathan/code/rust/aicacia/of/target/debug/deps/unified_server-3151668e4144c159 --exact permission_http_tests::shared_identity_management_writes_change_idp_authorization
RUST_MIN_STACK=67108864 /home/nathan/code/rust/aicacia/of/target/debug/deps/unified_server-3151668e4144c159 --exact permission_http_tests::remote_identity_process_tests::independent_processes_share_remote_identity_and_consumer_reset_preserves_rows
```

Observed final results: Device authority plus remote process tests **10.75s** (the child entry without environment is a no-op); remote HTTPS Create/Join/reopen/reset **53.82s** including its explicitly launched child; shared identity **29.57s**; remote identity process acceptance **31.18s**. The reset test also proves that unexpected retained `shared-owner.redb` data refuses cleanup before successfully resetting the empty consumer root and observing two preserved Devices through the live source API.

Additional passed checks:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_INCREMENTAL=0 RUST_MIN_STACK=67108864 cargo run -p device-server --no-default-features --features cli --bin device-database -- --help
cargo fmt --all -- --check
python3 scripts/check_server_dependencies.py
git --no-pager diff --check
```

TDD red/green startup command:

```sh
cargo test -p device-server --no-default-features --lib remote_device_startup_never_falls_back_to_a_private_database
```

The first run failed because remote configuration opened a local owner; it passed after mode selection and now also checks a real unavailable remote connection. The process acceptance initially failed compilation at the missing remote startup/router seam. The reset safety acceptance failed because a valid consumer reset deleted unexpected owner data, then passed after the empty-root guard. Fixture fixes supplied the established OAuth `iss`, valid subject/application UUIDs, pinned peer locator and `offer_id`; none weakened production checks.

### Required broader checks — actual failures/limits

```sh
timeout 300s env CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_INCREMENTAL=0 RUST_MIN_STACK=67108864 cargo hack test --feature-powerset --all-targets -p device-service -p device-server
timeout 240s env CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_INCREMENTAL=0 RUST_MIN_STACK=67108864 cargo hack test --feature-powerset --all-targets -p device-server
timeout 180s env CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_INCREMENTAL=0 RUST_MIN_STACK=67108864 cargo hack test --feature-powerset --workspace --all-targets
timeout 180s env CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_INCREMENTAL=0 RUST_MIN_STACK=67108864 just crap
```

- Combined Device matrix exited 1 on the existing ticket02 `device_contract` fixture `approved_enrollment_denies_unresolved_owner_conflicts_even_with_exact_visible_binding`: `Incomplete catalog: data table devices has no definition` at line 225, under `--no-default-features --features default,replica,utoipa`. All 56 unit tests in that combination passed; 5/6 contract tests passed. The matrix stopped before qualifying all combinations or Device-server.
- The separate Device-server matrix exited **124 at 240s**, during its feature-enabled test run after compilation. Several tests and explicit HTTPS children passed, but the matrix did not complete; no powerset pass is claimed.
- Workspace matrix passed combinations 1–16, including all 11 `idp-db` combinations, then exited 1 at **17/177**, `cli --no-default-features`: `clap_builder` requires its `std` feature. This unrelated preexisting feature-wiring failure was not changed.
- `just crap` exited **124 at 180s** during workspace coverage compilation. No complete coverage output or CRAP score was produced.
- One setup run timed out at the existing 30-second IdP readiness bound; an unchanged executable rerun reached the intended reset red, and final source acceptance passed at unchanged bounds. Latency/reliability SLA is not claimed.
- Process admission uses production SQL startup/query/router/Storage-admission functions inside independent test-executable children and a controlled IdP introspection/token fixture. It proves endpoint admission through a test HTTP probe, not a full `storage-server` CLI or resource iroh session. The separate remote setup acceptance exercises actual OAuth and Storage HTTP with isolated mock keyring/native roots and OS-trust command stubs. Both use explicit 64 MiB stacks. Real OS provider/trust installation, default-stack, application mTLS rollout, GUI, WAN/container and other-platform deployment remain unqualified. Existing ofdb mTLS/conflict parity evidence is ticket03's `59ac5a5`, not a new sibling-repository rerun here.

### Integration handoff

Implementation commit: **`808212aa8` — Serve remote Device SQL and provision shared control-plane consumers**.

After committing, ran these exact commands in worktree `04`:

```sh
GIT_EDITOR=true git merge integration/shared-control-plane-databases
git --no-optional-locks status --short --branch
git --no-pager log -2 --oneline
git --no-pager rev-parse integration/shared-control-plane-databases
```

Observed: **Already up to date**, clean `implement/shared-control-plane-04`, integration tip **`ba45e15f52fb19b2519b602cbd733684d8691377`** already an ancestor. This completion evidence is a documentation-only follow-up commit, with the merge repeated afterward. No merge into the original checkout was performed; the merger owns that handoff. No new ofdb changes or commits were made.
