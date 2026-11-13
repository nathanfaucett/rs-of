# Ticket 06 validation

Status: incomplete; ticket remains claimed.
Branch: `backend-simplification-cleanup`.
Base: integration `64e44f11e`.
Worktree: `.scratch/worktrees/cleanup`.

## Scope and risk

Low complexity: remove one unused direct dependency and document existing boundaries.
High risk if required independent setup paths or retained user data are removed.
Priority: preserve independent owners and report strict acceptance gaps.

No current Rust implementation, frontend/E2E files, `.github` files, or `setup`
crate content was changed. No ignored or untracked data was removed.

## Audit and changes

- Ticket 04 (`64e44f11e`) already deleted the old unified setup host and lifecycle files. All Rust files in `crates/unified-server` contain none of `SetupHost`, `IndependentCreateHost`, `SetupAdmission`, `SetupCapability`, or `LocalTls`.
- `api`, `idp-model`, `storage-model`, and `management-model` are already absent from disk, tracked paths, and workspace members. Their model extraction was committed in `fe234418e`.
- `git ls-files data/unified` is empty, and this worktree has no `data/unified` directory. There are no tracked obsolete unified data files left to delete. No deletion of another worktree's ignored data is justified.
- Removed Management's unused direct `setup` dependency and its Cargo.lock edge. Source search and cargo-machete agree that this edge is unused. The root workspace `setup` dependency remains required by IdP, Device, and Storage.
- The focused dependency scan reports no unused dependencies in unified-server or server-common. It also reports Management's `utoipa-swagger-ui`; this is unrelated to legacy setup and remains unchanged.
- Updated README, the shared glossary definition, glossary map, and deployment-boundary documentation. Local composition does not imply independent Node setup or mesh membership.
- There is no further safe net file deletion in this scope. The significant legacy deletion belongs to prior tickets, not this commit.

## Setup exclusion feasibility

`cargo tree -p unified-server -e normal -i setup --offline` still shows these normal dependency paths:

- unified-server -> idp-server -> setup
- unified-server -> device-server -> setup
- unified-server -> storage-server -> setup
- unified-server -> owner servers -> server-common -> setup
- owner servers -> device-service -> server-common -> setup

The removed Management direct edge is gone, but its server-common path remains.
IdP, Device, and Storage compile independent startup/Create/Join/reset modules
that use setup. server-common re-exports setup types and shared helpers such as
canonical OpenAPI metadata, which Management uses too. Independent LocalTls
clients remain in owner modules. These are live independent paths, not obsolete
unified imports.

Cargo workspace exclusion does not remove a required path dependency. Making
setup optional only in a manifest would break these compiled modules. Full
exclusion requires a coordinated owner/common feature or module split, including
all required dependencies and callers. That changes current Rust implementations
and exceeds this task's ownership. No required path, export, or check was removed
to satisfy a tree assertion. Independent setup contracts also remain necessary
for OpenAPI export.

## TDD and validation

Loaded the TDD skill. Used the task's existing Cargo dependency/build and source
acceptance audits; added no test at a new seam and changed no runtime behavior.
Before deletion, the dependency scan found Management's unused direct setup edge.
After deletion, it no longer reports that edge, and the inverse tree confirms its
removal. This is an audit-before/audit-after check, not a passing runtime TDD cycle.

### Interrupted-session record (retained)

The following results were already recorded in the interrupted session. The resumed
checks below are separate evidence, not a claim that every earlier result was
reproduced.

Commands ran offline where Cargo supports it. Build commands used the shared
`of/target` via `CARGO_TARGET_DIR`; concurrent builds caused lock waits.

| Check                                                                               | Actual result                                                                                                                                                                                                         |
| ----------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tracked-path, filesystem, workspace-member, and unified Rust-symbol assertions      | Passed. No obsolete paths or setup symbols remain in unified-server.                                                                                                                                                  |
| `git diff 64e44f11e -- crates/setup` and setup tree comparison                      | Empty diff; unchanged.                                                                                                                                                                                                |
| `cargo tree -p unified-server -e normal -i setup --offline`                         | Ran successfully; strict no-setup acceptance **fails**, with required transitive paths above.                                                                                                                         |
| `cargo machete crates/unified-server crates/server-common crates/management-server` | Management setup finding removed; unrelated Management Swagger finding remains.                                                                                                                                       |
| `cargo check -p setup --lib --offline`                                              | **Passed**, with existing warnings.                                                                                                                                                                                   |
| `cargo check -p setup --all-targets --offline`                                      | **Failed**, five existing setup test compile errors; see below. Initial attempt timed out after 180 seconds; another attempt waited on build locks and timed out after 60 seconds.                                    |
| `cargo test -p setup --all-targets --offline`                                       | Timed out after 200 seconds on build locks; no tests passed.                                                                                                                                                          |
| `cargo check --workspace --offline`                                                 | **Failed**, four `KvSnapshot` serde errors in sibling offs/ofdb. Initial attempt timed out after 200 seconds on locks.                                                                                                |
| `cargo hack check --feature-powerset --all-targets --offline`                       | Timed out after 200 seconds during compilation. Retry timed out after 70 seconds. No complete result.                                                                                                                 |
| `cargo hack test --feature-powerset --workspace --all-targets --offline`            | Timed out after 60 seconds compiling the first package. No full suite result.                                                                                                                                         |
| `just --no-deps crap`                                                               | Timed out after 60 seconds during coverage dependency compilation. No CRAP result. `--no-deps` skips `pnpm sdk:build` to preserve the frontend ownership boundary; `just --dry-run crap` confirmed that prerequisite. |
| `git diff --check`                                                                  | Passed before commit.                                                                                                                                                                                                 |

Setup test compile errors, in unchanged files:

- `crates/setup/src/lib.rs:77`: duplicate `independent_exchange_lifecycle_tests` module.
- `crates/setup/src/independent_exchange_lifecycle_tests.rs:16`: unresolved `Challenge` and `OutstandingChallenge` imports.
- `crates/setup/src/setup_server_teardown_tests.rs:240`: missing `super::serve_listener`.
- The same file, lines 245 and 252: `.expect()` called on an unresolved/incompatible result.

Additional source audit: the independent IdP, Device, and Storage
`src/startup_https_tests.rs:5` still point to the deleted
`../../api/src/setup_https_fixture.rs`. The fixture exists at
`crates/setup/src/setup_https_fixture.rs`. These live owner tests need a path
repair by their owner; they were not deleted, bypassed, or changed here. The
unchanged setup crate also retains a historical `api::serve_setup_verified`
compile-fail doc example. Neither is a unified runtime reference.

## Acceptance gaps

- Setup is still transitive. Strict graph exclusion is not feasible within this ownership boundary.
- Workspace check fails on the sibling snapshot serde mismatch. No full feature or test matrix passed.
- The setup library builds, but its test targets do not. Independent deployment behavior is not requalified.
- Broken owner HTTPS fixture paths remain for their owner to fix.
- There are no remaining tracked obsolete files to delete, so this commit cannot supply a new significant file-count reduction.

Do not mark ticket 06 resolved on this evidence.

## Resumed verification

Read the spec and ticket 06, loaded the TDD skill, and preserved all nine existing
modified or new files. No new runtime test seam was introduced. Scope remained
cleanup audits, Cargo metadata, and documentation. No additional deletion was
needed: every spec-listed obsolete path was already absent in this worktree.

Resumed Cargo checks and tests used `--locked --offline` and the shared
`of/target`. The CRAP recipe used `CARGO_NET_OFFLINE=true` and that same target.

| Check                                                                                                            | Resumed result                                                                                                                                                                                          |
| ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Filesystem and tracked-path assertions for `data/unified`, the four deleted crates, and obsolete unified modules | Passed. No data was deleted.                                                                                                                                                                            |
| Workspace and Cargo.lock assertions                                                                              | Passed. Deleted crates have no manifest path/package references or lockfile entries; Management has no direct setup lockfile edge.                                                                      |
| Unified Rust symbol audit                                                                                        | Passed for `SetupHost`, `IndependentCreateHost`, `SetupAdmission`, `SetupCapability`, and `LocalTls`.                                                                                                   |
| Setup preservation                                                                                               | Empty `git diff 64e44f11e -- crates/setup`; setup tree matches the base. No Rust, frontend, E2E, or setup edits.                                                                                        |
| `cargo tree -p unified-server -e normal -i setup --locked --offline`                                             | Command passed; strict no-setup acceptance still fails. Required owner/common transitive paths remain.                                                                                                  |
| `cargo machete crates/unified-server crates/server-common crates/management-server`                              | Exit 1 for the unrelated Management `utoipa-swagger-ui` finding only. No setup finding.                                                                                                                 |
| `cargo check -p setup --lib --locked --offline`                                                                  | Passed with warnings after an initial 180-second compilation timeout.                                                                                                                                   |
| `cargo check -p setup --all-targets --locked --offline`                                                          | Failed with three errors: duplicate lifecycle test module, unresolved challenge imports, and inaccessible `serve_listener`. The earlier five-error record above is historical; this run reported three. |
| `cargo check --workspace --locked --offline`                                                                     | Failed with four `KvSnapshot` serde errors in sibling filesystem dependencies after an initial 180-second timeout.                                                                                      |
| `cargo hack check --feature-powerset --all-targets --locked --offline`                                           | Timed out after 60 seconds in the first package's all-features build. No complete matrix result.                                                                                                        |

Additional final checks:

- `cargo hack test --feature-powerset --workspace --all-targets --locked --offline`
  timed out after 60 seconds compiling the first package (`of-idp-client`, 1/200).
  No tests completed.
- `CARGO_NET_OFFLINE=true just --no-deps crap` timed out after 60 seconds compiling
  coverage dependencies. No CRAP result. `--no-deps` avoids the frontend
  `pnpm sdk:build` prerequisite confirmed by `just --dry-run crap`.
- `git diff --check` passed.

The three independent owner HTTPS tests still reference the deleted API fixture
path. These Rust fixes remain with the buildfix owner. Neither the dependency
audit nor the setup library build qualifies independent deployment behavior.

Commit hooks are bypassed to prevent broad Rust/frontend mutations outside this
scope. The explicit checks above, not the commit, are the validation evidence.
