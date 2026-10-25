# Final boundary cleanup and combined basic validation

## Scope and integrated state

The clarified goal is **correct code/test ownership and deletion of obsolete code,
not full test parity**. Keep unique recovery, rejection, revocation, secret-isolation
and retained-operation assertions. Delete duplicated happy paths; do not expose
production internals, introduce compatibility wrappers, or manufacture test-only
production features merely to move tests.

This final pass fast-forwarded `refactor/boundary-host-tests` to integration
`64c98b5c7`. That tip contains the owner, Device security, external full-node,
fixture-hardening and forwarding-wrapper deletion changes. All final checks below
ran against this combined worktree, with the two confirmed-unused Unified dev
edges removed. They are not a collection of checks from unmerged agent branches.
The only final-pass code/configuration changes are the Unified dev-dependency
removal and its two corresponding `Cargo.lock` dependency entries. This report
is new. No root manifest, feature flags, test assertions, or production behavior
changed in this final pass.

## Actual boundaries and deletions

| Area | Final location and boundary |
| --- | --- |
| Device security history, candidate admission and synchronization | `crates/device-service/src/security_history_tests.rs`, `security_candidate_tests.rs`, `security_sync_tests.rs`; genuine owner-private tests use Device's own cfg(test) fixtures, not Unified. Invalid retained history is injected at the owning boundary. |
| Resource cleanup/retirement | `crates/device-service/tests/owner_cleanup.rs`, using the public owner contract. |
| Device authority, permission HTTP, native Storage/IdP authorization, residency, policy cleanup, Node bootstrap/interruption | Root `tests/owner_boundaries.rs` and `tests/owner_boundaries/`, with public fixtures under `tests/support/`. This target compiles and runs without root default features and has no Unified dependency. |
| Owner composition/transfer/reset/restart | Root `tests/owner_composition_tests/`, included by `owner_boundaries`. Host-specific identity composition lives separately in `tests/unified_identity_composition.rs`, a CLI-required target. |
| Full-node bootstrap, partition/offline reopen, learned refresh denial and retained-socket restriction | `crates/unified-server/tests/full_node.rs`, using public, fixture-owned `RuntimeOwners` from `tests/support/full_node.rs`; no private Runtime accessors. |
| Common actual-process lifecycle and SDK scenarios | Existing root shared system scenarios remain authoritative for common Create/restart/Join/use. They were not rewritten or rerun as part of this final basic pass. |

Concrete cleanup:

- Deleted Unified's `src/security_sync.rs` (96 lines): forwarding newtype, `Deref`,
  constructor/factory, stream attachment and runtime-owner adapter. `lib.rs`
  directly reexports Device's `SecurityContext` and `SecurityProtocol`.
- `runtime::compose_protocol_router` directly attaches the owner's
  `accept_data_stream` closure. `local_join.rs` and the external full-node join
  fixture construct the actual owner protocol at their call sites, with no new
  public replacement helper. `check_bootstrap_endpoint` still runs before opening
  owners; database identities, staging path, cancellation and subsequent readiness
  checks are retained. `into_databases()` becomes explicit `RuntimeOwners`.
- The forwarding-wrapper cleanup commit `ddd09bbf7` removed 115 lines and added
  55 across its five files: **60 net lines removed**, including the external test
  call-site change.
- Deleted the 203-line `full_node_probe.rs`; its refresh/replay and restriction
  assertions are covered by retained full-node cases. Deleted the redundant
  bootstrap wrapper that reran the initial/offline path while skipping the
  additional security assertions, and the HTTPS local-create-only wrapper whose
  assertions are retained in the Create/Join case.
- Removed the runtime HTTP trace hook and every caller, the redundant identity
  accessor, private declarations for moved tests, and the old rotation-client
  helper from the private provisioning suite.
- External full-node roots now use `api::setup_files::create_private_directory`
  (0700), with mode-0600 password input. A regression reproduced the old 0777 root
  under umask 000 before passing after the fix. The namespace adapter discards
  unsupported mock-provider modifiers and rejects unknown namespace prefixes;
  its focused regression passes. Removed redundant per-Fixture global provider
  installation and the single-use OAuth helper. Scenarios serialize their
  process-global provider and use 64 MiB stacks with a 600-second scenario bound.
- Ordinary full-node Device restrictions, Storage CRUD, User updates and JWKS
  reads use generated clients. Raw OAuth and socket operations remain where
  response/replay/framing semantics are the assertion.

### Measured source/test inventory

Compared with base `536975ca0`, at integration `64c98b5c7` plus the final dev-edge
trim (which does not change Rust source). Counts are tracked `.rs` files, physical
lines including blanks/comments, and source attributes matching `#[test]` or
`#[tokio::test(...)]`. They include ignored/cfg-specific tests and helper files;
they are **not executed test counts or a parity claim**.

| Tracked area | Files before → after | Lines before → after | Test attributes before → after |
| --- | --- | --- | --- |
| `crates/unified-server/src` | 49 → 24 | 30,149 → 12,560 | 143 → 67 |
| `crates/unified-server/tests` | 5 → 7 | 891 → 2,432 | 10 → 15 |
| `crates/device-service/src` | 39 → 42 | 10,958 → 13,232 | 68 → 87 |
| `crates/device-service/tests` | 2 → 3 | 354 → 688 | 8 → 10 |
| Root `tests` | 4 → 31 | 2,059 → 15,896 | 10 → 61 |

Unified's source subtree shrank by 17,589 lines, primarily because tests now live
at their owning seams. Across these five tracked areas Rust source grew by 397
lines overall (44,411 → 44,808), including public fixtures and additional targeted
regressions. Moving tests is not itself repository-wide code reduction; the
concrete deletions above, direct owner API and removed dependency edges are the
actual simplifications. No blanket net-reduction or complete parity claim is made.

## Dependencies and feature isolation

Confirmed by searches across all Unified Rust sources/examples/tests and its
included root identity fixture, then by all-target compilation:

- Remove Unified dev-dependency `file-system`: no direct caller remains.
- Remove Unified dev-dependency `postcard` and the obsolete wire-assertions header:
  no direct caller remains after the security/bootstrap moves.
- Keep `storage-service`: private runtime tests still construct `DatabaseRuntime`
  and `ScopedFileSystemRuntime`. Keep `storage-model`, socket helpers, `tower`,
  `http`, keyring fixtures and the OpenAPI example's `utoipa`, all with real callers.
- Keep dev `api/test-support`: host TLS/trust/verifier fault tests still use it.
- Only the two Unified package edges disappear from the lockfile. The packages
  themselves remain legitimately needed by other owners/root tests.
- The complete root `--no-default-features` normal/dev dependency tree contains
  no `unified-server` entry (checked programmatically, not inferred from imports).
- Root `--no-default-features --tests`, including `system_create`, passes. Its
  system runner does not directly reference disabled Unified code, so no cfg
  gating change is necessary. The actual Unified launcher remains CLI-required.

## Final combined basic validation

Commands used the worktree manifest, shared target `../of/target`, `-j8` and
`--offline` for builds/tests/checks. The main combined check/test groups used 600-second bounds; preparatory checks
and Clippy used 120-second bounds. No final combined run timed out. Focused runtime tests set
`RUST_MIN_STACK=67108864`. Package formatting/diff checks do not need Cargo build
flags. All rows below passed on the combined final state.

| Command, omitting the common manifest/target/jobs/offline options | Result |
| --- | --- |
| `cargo check -p unified-server --all-targets` | Default features, passed. |
| `cargo check -p unified-server --features cli --all-targets` | CLI features, passed. |
| `cargo check -p idp-unified --no-default-features --test owner_boundaries` | Passed without Unified. |
| `cargo check -p idp-unified --no-default-features --tests` | Passed, including compilation of `system_create`; no gating fix needed. |
| `cargo check -p device-service --no-default-features --lib` | Device no_std configuration passed. |
| `cargo check -p device-service --no-default-features --features replica --tests` | Replica unit/integration test targets, including `owner_cleanup`, passed compilation; not a suite execution. |
| `cargo test -p unified-server --test bootstrap_config --test composition --test owner_routes --test setup_host -- --test-threads=1` | 7 passed: 2 + 2 + 1 + 2. |
| `cargo test -p unified-server --test http_host fresh_host_publishes_complete_openapi_without_provisioning -- --exact --nocapture` | 1 passed, 2.67 s. |
| `cargo test -p idp-unified --no-default-features --test owner_boundaries native_api_tests::native_local_token_read_response_denies_learned_revocation_before_transmission -- --exact --nocapture` | 1 passed, 8.80 s; actual moved owner/native revocation boundary. |
| `cargo clippy -p unified-server -p device-service --lib --no-deps -- -D warnings` | Strict affected production-library lint passed; Device replica is selected through Unified's dependency. No generated dependency lint detour. |
| `cargo fmt --package unified-server --package device-service --package idp-unified -- --check` | Passed, including moved modules. |
| `git diff --check` | Passed. |

Compilation still reports pre-existing unused shared-test helpers in the Unified
lib-test target and unused imports in IdP's no_std configuration. They are not
production Clippy failures and were not changed to silence unrelated diagnostics.

## Earlier valid evidence carried forward, not rerun as a suite

| Evidence and tested state | Result / provenance |
| --- | --- |
| External full-node consumption/revocation, retained-socket restrictions, partition/offline reopen at fixture-hardening `4471f8aeb` | All three passed individually: 142.24 s, 139.19 s, 146.17 s. 600-second bounds, 64 MiB stacks. |
| Consumption/revocation after direct owner reexport/construction at `ddd09bbf7` | Passed again, 124.94 s; exercises both public imports and the directly composed DATA synchronization handler. The other two were not rerun after wrapper deletion. |
| External fixture permissions and modifier/namespace regressions | Both passed under umask 000 at `4471f8aeb`; the permission regression had first failed as expected. |
| Moved owner bootstrap/offline retry and actual built Rust/JavaScript SDK operations at integration `9cd44cf6d` | Passed, 168.50 s and 12.14 s, respectively; see [owner-boundary-validation.md](owner-boundary-validation.md) for commands, artifact prerequisites, environment-failure correction and limits. |
| Device security cases following `da4f478d0` | Integration lead reports 17 passed. This is carried-forward integration evidence, not a new execution or independently reconstructed test log in this pass. |

Earlier 120-second incomplete full-node attempts are not counted as passes. The
mock-provider modifier error was fixed before the complete runs. Earlier default
stack overflows were addressed by the explicit fixture stack sizes; the final
passing cases above are the evidence, not the failed attempts.

## Why genuinely host-private tests remain

- `local_provisioning_tests.rs` checks exact guarded journals, verifier retention,
  private-inventory interruption/readback, missing signer/context roots, durable
  publication and no-recreation recovery. Moving these by exposing private
  journals or inventing public inspection APIs would weaken the boundary.
- `hosted_setup_integration_tests.rs`, nested `https_bootstrap_tests.rs` /
  `api_join_tests.rs`, and `setup_runtime_tests.rs` retain host publication,
  cancellation/finalization, retained admission, trust/native-root isolation,
  current readiness and recovery checks involving private worker/lifecycle state.
- `endpoint.rs`, `runtime_peer_refresh_tests.rs`, configuration/CLI/origin unit
  tests and the private runtime tests exercise host endpoint publication,
  allowlist refresh, local listener/prefix/lifecycle composition and configuration
  invariants. Some runtime listener acceptance coverage is still mixed with owner
  permission checks via the shared identity fixture: this is a residual mixed
  host test, not proof that every duplicate has been removed.
- The remaining `full_node_runtime_fixture.rs` Device/server accessors and
  `node_join_source_for_test` still have callers in the private HTTPS rejection,
  acceptance-expiry and retained-offer-release checks. They are not used by the
  external full-node suite. Unused identity/tracing hooks were deleted; live hooks
  were not hidden behind a new production feature or widened visibility.

No expensive nine-process lifecycle execution, full library/workspace suite,
feature powerset, CRAP run, new production feature, main/integration merge back,
or push was performed in this final pass. Genuine private regressions were not
deleted to manufacture parity or an attractive line-count claim.


## Follow-up P2 review: shared fixture credential callers

Review of `95d1d9cf4` found that `tests/support/identity.rs` registers
`storage-device` / `storage-device-secret`, but the private Unified listener
acceptance test still requested `storage-management` / `storage-management-secret`
for endpoint lookup and replication-selection reads. Updated both callers in
`crates/unified-server/src/runtime.rs` to the registered credentials: four string
literals, with no compatibility alias or fixture registration change. All existing
assertions, requested audiences and Management-prefixed permission scopes remain.

A repository-wide search of the remaining old-name uses found no other stale
callers of this shared fixture. Kept the independent provisioning test's
`storage-management.json` filenames, the obsolete-config rejection case's
`storage_management` key, and JavaScript mock token labels. These do not request
the removed shared-fixture Client. Other actual Management client names remain
unchanged.

Focused validation on this worktree, after the fix:

```sh
RUST_MIN_STACK=67108864 cargo test -p unified-server --lib \
  runtime::tests::builds_three_prefixed_services_with_one_endpoint \
  --target-dir ../of/target -j8 --offline -- --exact --nocapture
```

**Passed: 1 test, 102.82 seconds**, with a 600-second subprocess bound. The complete
listener, endpoint/readiness, Join approval/rejection/replay, service-credential
and permission assertions ran unchanged. The existing SDK consumer also printed
`Built Node-join export qualified against actual host-composed router`.

The first attempt stopped earlier at the Node-join consumer because this
worktree lacked the built Device SDK (`MODULE_NOT_FOUND`); it is not counted as
a pass. The successful rerun used existing main-checkout build artifacts read-only
through temporary worktree symlinks at `node_modules` and
`packages/device-client/esm`, removed in a `finally` block. No main file, staged
change, package export, test assertion or qualification script was modified.
Unified package formatting and diff checks also passed. No full suite, CRAP,
feature powerset, main/integration merge or push was performed for this fix.
