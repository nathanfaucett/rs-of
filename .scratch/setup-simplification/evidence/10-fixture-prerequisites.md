# Ticket 10: validation fixture prerequisites

## Ownership, base and limits

- Worktree: `/home/nathan/code/rust/aicacia/of/.worktrees/setup-10`; branch: `implement/setup-followup-10`.
- Confirmed clean initial HEAD and integration tip: `da5bc63419623ab48f93dea14dfce20f3e10ffc9`; merge-base with requested `da5bc6341` was that same commit. Ticket claimed before implementation.
- Delivery integration check: `GIT_EDITOR=true git merge --no-edit integration/setup-simplification-followup` returned `Already up to date.`; observed integration tip still `da5bc63419623ab48f93dea14dfce20f3e10ffc9`. No integration branch movement or provider implementation changes were made here.
- Read root AGENTS, issue-tracker instructions, shared glossary/map, setup-simplification spec and ticket 07 validation diagnosis. Loaded TDD, ponytail and GitHub Actions guidance.
- Owned SDK prerequisite test/CI entry points, reset fixture tests, ticket 10 and this receipt. No map, other ticket, production provider ownership, real profile, provider or trust changes. Existing newer SDK implementation remains unchanged.
- Complexity/risk/priority: low implementation complexity, safety-sensitive reset assertions, routine priority. Reuse the existing exported-package/router and isolated Create/Join/reset seams; no new dependencies or migration support.
- Initial source-only pass: ticket 08 owned the Rust build slot, so changed Rust tests were not compiled/executed then. The later **exclusive-slot follow-up below supersedes that validation limit**; no matrix or coverage was run in either pass.

## Changes

- `pnpm sdk:build` builds only the four owner-client ESM packages consumed by the existing exported-JavaScript scenario, using existing `build:esm` scripts. The general `packages:build` and newer `@of/sdk` work are unchanged.
- `just test`, `just crap` (including `crap-summary`) and `just cov` explicitly depend on that build. Cargo-test CI installs Node/pnpm and frozen JS dependencies before its existing recipe. No events, permissions, cache scope or trust settings changed. Workflow syntax/step-order reference: GitHub Actions skill `references/workflows-and-actions/workflow-syntax.md`.
- The direct Rust SDK test checks the five required ESM entry files **before creating the backend fixture**, with an actionable installation/build command. It still executes the real Node consumer and asserts success; missing exports are never skipped.
- OpenAPI qualification already builds those packages, but its old `unified-server --lib generated_` filter no longer addressed the root integration scenario. It now explicitly selects `idp-unified --test owner_boundaries native_api_tests::generated_rust_and_built_javascript_use_actual_owner_routes -- --exact`.
- Replace the obsolete legacy-reset fixture with unsupported-record refusal followed by cleanup using exact original current records. Renamed Create/Join parents and their private environment switch accordingly; no compatibility aliases.
- Negative fixtures separately cover unsupported Device v1 / IdP v255, historical unknown fields, and malformed JSON. Ordinary startup and reset must refuse; provider inventory, worker, host, CA and sentinel local data remain unchanged. A captured reset journal must remain in `secrets`, contain the exact injected worker, and reject a conflicting current worker without overwriting evidence. Only then does test-fixture teardown discard its own injected journal and restore its original bytes. This is **not** a supported production recovery/migration path.
- Current-record cleanup retains provider-unavailable and partial endpoint-deletion retry checks, now requiring the actual injected error rather than accepting any parser failure. Provider refusal must preserve inventory. Existing unrelated/foreign-slot protection, per-owner boundaries, data-only retry, exact-reference recovery and peer-revocation tests remain; no production reset semantics changed.

## Executed results

Environment: Node `v26.5.0`, pnpm `12.4.1`.

| Command                                                                                                                                                                                                          | Exact outcome                                                                                                                                              |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm install --frozen-lockfile --ignore-scripts`                                                                                                                                                                | Passed; 13 workspace projects, 465 packages reused, no downloads.                                                                                          |
| `node --test scripts/generated-package-exports.test.mjs` before build                                                                                                                                            | Exit 1: missing ESM exports, including `packages/idp-client/esm/index.js` and Storage generated entry. Confirms prerequisite red.                          |
| `pnpm sdk:build`                                                                                                                                                                                                 | Passed; IdP, Management, Device, Storage ESM compilation. No backend compilation.                                                                          |
| `node --test scripts/generated-package-exports.test.mjs` after build                                                                                                                                             | **5 passed**, 0 failed/skipped.                                                                                                                            |
| `node --test scripts/generated-clients.test.mjs scripts/generated-device-client.test.mjs scripts/generated-independent-client.test.mjs scripts/generated-setup-client.test.mjs scripts/owner-contracts.test.mjs` | **30 passed**, 0 failed/skipped.                                                                                                                           |
| Existing executable SDK scenario, detailed below                                                                                                                                                                 | **1 passed**, 49 filtered out, 0 ignored/failed; 12.50 s test time, 12.55 s supervised, exit 0.                                                            |
| `pnpm --filter @aicacia/storage-client exec vitest run --maxWorkers 1 --no-file-parallelism`                                                                                                                     | **21 passed / 2 files**.                                                                                                                                   |
| `pnpm --filter @of/sdk exec vitest run --maxWorkers 1 --no-file-parallelism` initially                                                                                                                           | Failed to import missing built `@aicacia/native-fetch`; no tests executed. No SDK source repair attempted.                                                 |
| `pnpm --filter @aicacia/oidc-client exec vitest run --maxWorkers 1 --no-file-parallelism` initially                                                                                                              | Same missing native-fetch prerequisite; no tests executed.                                                                                                 |
| `pnpm --filter @aicacia/native-fetch run build:esm`                                                                                                                                                              | Passed using existing TypeScript build only.                                                                                                               |
| Same SDK Vitest command after native-fetch build                                                                                                                                                                 | **45 passed / 1 file**, confirmed again independently.                                                                                                     |
| Same OIDC Vitest command after native-fetch build                                                                                                                                                                | **15 passed / 1 file**.                                                                                                                                    |
| `rustfmt --edition 2024 --check tests/owner_composition_tests/reset.rs tests/owner_composition_tests/create.rs tests/owner_composition_tests/join.rs tests/owner_boundaries/native_api_tests.rs`                 | Passed; formatting/parsing only, not type checking.                                                                                                        |
| `node --check scripts/qualify-openapi-clients.mjs`; `git diff --check`                                                                                                                                           | Passed.                                                                                                                                                    |
| `just --dry-run test`, `just --dry-run crap`, `just --dry-run cov`, `just --dry-run crap-summary`                                                                                                                | Passed. Direct recipes print `pnpm sdk:build` before Rust commands; summary prints its nested `just crap --summary`. No Rust command executed by dry runs. |
| PyYAML parsing and assertions on `.github/workflows/ci.yml`                                                                                                                                                      | Passed: frozen dependency install precedes test recipe; permissions still `contents: read`. `actionlint` unavailable; no hosted CI execution claimed.      |

### Existing executable scenario provenance

Executed only:

```text
/home/nathan/code/rust/aicacia/of/target/debug/deps/owner_boundaries-f196021ee7efba92 native_api_tests::generated_rust_and_built_javascript_use_actual_owner_routes --exact --nocapture --test-threads=1
```

This existing executable embeds the main worktree manifest path, not setup-10. An ephemeral private `node` shim in a temporary PATH redirected **only the script filename** to setup-10's `scripts/qualify-built-openapi.mjs`, forwarding the executable's three real fixture router URLs unchanged to the real Node executable. No script assertions, generated code, backend behavior, TLS or provider policy were mocked or bypassed. The shim was removed in `finally`; the process had a 90-second bound. Output: `Built package exports and generated operations qualified against actual owner routers`.

This proves the actual owned-worktree exported JavaScript scenario against the **pre-existing backend executable**. It does not prove that executable corresponds to the current integration source, ticket 08, or the newly edited Rust prerequisite assertions. Rebuild/rerun remains a follow-up gate.

### Incomplete ancillary attempt and cleanup

An initial 30-second combined Node command also included `scripts/generate-openapi-client.test.mjs` alongside the six export/contract files. It **timed out**, so no result is claimed for that combined invocation or generator-failure suite. That test can invoke the SDK generator and eventually Cargo; it should not have been included in this build-slot-restricted pass. At inspection its remaining stage `.openapi-generation-uMioYx` contained only copied TypeScript sources and an **empty `rust/`**, no stage target or Cargo.lock; generator source places Rust generation before Cargo validation. No owned generator descendants remained visible. Removed that owned stage, did not rerun the generator suite, and reran only the bounded non-generating contract checks listed above.

Frozen installation/ESM/Vitest commands changed tracked Storage source-map/node_modules artifacts. Restored **only those changes produced by this pass** from the initially clean checkout. No generated output, node_modules, staging tree, lock refresh, main-worktree artifact, or new dependency belongs in the delivery commit.

## Initial handoff plan (superseded by executed follow-up below)

**Ready for Rust validation; wait for ticket 08 to release the shared build slot.** Run from setup-10 with `CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target`, serially. First ensure the frozen JS installation and `pnpm sdk:build` prerequisites. Do not invoke ignored children directly: parents establish private roots, isolated native CA bundles, no-op trust commands, mock providers and 180-second child deadlines.

Focused commands (each line is a separate bounded run):

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target cargo test -p idp-unified --test owner_boundaries native_api_tests::generated_rust_and_built_javascript_use_actual_owner_routes -- --exact --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target cargo test -p idp-unified --test owner_boundaries owner_composition_tests::create::production_independent_create_unsupported_record_refusal_and_current_reset -- --exact --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target cargo test -p idp-unified --test owner_boundaries owner_composition_tests::join::production_independent_join_unsupported_record_refusal_and_current_reset -- --exact --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target cargo test -p idp-unified --test owner_boundaries owner_composition_tests::create::production_independent_create_and_offline_reopen -- --exact --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target cargo test -p idp-unified --test owner_boundaries owner_composition_tests::join::production_independent_join_and_source_offline_reopen -- --exact --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target cargo test -p idp-unified --test owner_boundaries owner_composition_tests::remote_setup::remote_create_join_reopen_and_device_reset_preserve_shared_installation -- --exact --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target cargo test -p lidp --lib setup_reset_tests -- --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target cargo test -p lidp --lib setup_peer_revocation_tests -- --test-threads=1
```

After coordinated focused validation, broader gates remain `cargo hack test --feature-powerset --all-targets`, relevant clippy, `just crap`, and full `pnpm openapi:check --committed`. Do not overlap these with ticket 08's builds or infer their success from JS tests. Ticket 07's previously recorded provider/readiness/permission timeout and coverage limits remain unresolved here.

## Initial acceptance status (superseded)

The source-only handoff left the ticket claimed pending coordinated Rust validation. The following current-source execution closes that ticket-specific gap; the older existing-binary result above is not used as substitute evidence.

## Exclusive Rust-slot follow-up

### Integration and execution boundaries

- User released ticket 08's slot and granted ticket 10 exclusive Rust build/test ownership; agent 09 remained source-only. Merged `integration/setup-simplification-followup` by fast-forward to **`8ee26fb1f8d789840dc246b94cfa22f6fdb7e6a5`**, including ticket 08's provider changes and integrated ticket 10 fixture edits. All runs below rebuilt/used this current setup-10 source; no Node shim or pre-existing-executable workaround.
- Sibling HEADs observed after execution: ofdb `e3242170c94d5fc4bcc121db812310c8106c7ab6`, ofnet `dbea925a0a3f99053fffc02618baccc2a1cbea9f`, offs `df4c696a910296635473ec296d9235acbaa60d9b`. No sibling edits.
- Every Cargo invocation used the literal environment prefix `CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=2 RUST_MIN_STACK=67108864`. Rust commands ran serially, with `--locked` for tests and `--test-threads=1`. No matrix, coverage, CRAP, full OpenAPI generation, or Clippy run in this authorized focused pass.
- Bounds: SDK rebuild/run and initial native rebuild 600000 ms each; other owner-boundary parents 240000 ms each; corrected native safety groups 180000 ms each; test enumeration and formatting 60000 ms. No command timed out. Parent fixtures retained their own original 180-second child deadlines and launched their ignored children themselves.
- Native/independent scenarios used their existing private temporary roots, mock credential stores, isolated native CA bundles and injected no-op trust commands. No real profile/provider/trust changes. No production code or tests needed a follow-up repair; assertions were not weakened.

### Current-source results

First ran `pnpm install --frozen-lockfile --ignore-scripts` and `pnpm sdk:build`: both passed. Four owner-client ESM packages built successfully. The rebuilt Rust prerequisite check and actual Node consumer then executed through the ordinary test path.

All owner-boundary commands used `cargo test --locked -p idp-unified --test owner_boundaries <exact-name> -- --exact --nocapture --test-threads=1`, with the environment and bounds above. Exact selected names and actual outcomes:

| Exact selected name                                                                                              | Actual outcome                                                                                                                                                                                                                                                                                                    |
| ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `native_api_tests::generated_rust_and_built_javascript_use_actual_owner_routes`                                  | **1 passed**, 0 failed/ignored, 49 filtered out; 13.78 s test time after **7m14s current-source build**. Output: `Built package exports and generated operations qualified against actual owner routers`. Executable `target/debug/deps/owner_boundaries-fbdb13f2c672f60d`, compiled from setup-10.               |
| `owner_composition_tests::create::production_independent_create_unsupported_record_refusal_and_current_reset`    | **Parent and isolated child passed**; parent 128.46 s, child 128.34 s. Unsupported versions, historical unknown fields, malformed records, unchanged evidence/provider inventory, retained-journal conflict fencing, restored-current cleanup and provider/deletion failure retries all reached their assertions. |
| `owner_composition_tests::join::production_independent_join_unsupported_record_refusal_and_current_reset`        | **Parent and isolated child passed**; parent 102.10 s, child 101.98 s. Includes public Join, Storage use, source-offline reopen, negative retained-record cases and supported-current reset.                                                                                                                      |
| `owner_composition_tests::create::production_independent_create_and_offline_reopen`                              | **Parent and isolated child passed**; parent 127.86 s, child 127.70 s. Includes normal current-record cleanup and interrupted/provider-failure safety.                                                                                                                                                            |
| `owner_composition_tests::join::production_independent_join_and_source_offline_reopen`                           | **Parent and isolated child passed**; parent 96.77 s, child 96.58 s.                                                                                                                                                                                                                                              |
| `owner_composition_tests::remote_setup::remote_create_join_reopen_and_device_reset_preserve_shared_installation` | **Parent and isolated child passed**; parent 66.29 s, child 66.12 s. Remote consumer cleanup preserves shared owner data and other consumer authentication/signing.                                                                                                                                               |

Each owner-boundary parent invocation selected exactly one test (49 filtered out); child runs are additional nested fixture execution, not extra independent parent selections.

Native safety execution:

| Command after common environment prefix                                              | Actual outcome                                                                                                                                                                                                                                                                                                                  |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test --locked -p lidp --lib setup_reset_tests -- --test-threads=1`            | **0 tests selected**, 26 filtered out, after **7m03s rebuild**. This stale filename-derived filter in the initial handoff was incorrect and is **not counted as passing safety validation**.                                                                                                                                    |
| `cargo test --locked -p lidp --lib -- --list`                                        | Listed 26 current test names; confirmed actual module prefixes `setup_reset::tests` and `setup_peer_revocation::tests`.                                                                                                                                                                                                         |
| `cargo test --locked -p lidp --lib setup_reset::tests -- --test-threads=1`           | **6 passed**, 0 failed/ignored, 20 filtered out; 0.23 s. Covers damaged provisioning preservation, failed secret cleanup/retry, data-only retry without provider reuse, lost identity exact-reference recovery, symlink/conflicting recovery evidence refusal, retained Join exact owner IDs without endpoint/provider startup. |
| `cargo test --locked -p lidp --lib setup_peer_revocation::tests -- --test-threads=1` | **1 passed**, 0 failed/ignored, 25 filtered out; 0.08 s. Verifies issuer pinning, per-audience exchange and acknowledgment only after both owners.                                                                                                                                                                              |

Native executable: `target/debug/deps/lidp-d2096e17d9dad2bb`, rebuilt from setup-10. The initial planned `setup_peer_revocation_tests` filter was corrected before execution; it was not run as a zero-test substitute.

Additional checks: `node --test scripts/generated-package-exports.test.mjs` **5 passed** again; `cargo fmt --all -- --check` and `git diff --check` passed. `Cargo.lock` retained SHA-256 **`bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`**. Frozen pnpm installation changed only the seven tracked Storage node_modules launcher/symlink artifacts; restored those pass-owned changes before committing. No generated ESM/type output, lock changes or pnpm artifacts are delivered.

### Final disposition

**Ticket 10 resolved at its stated acceptance boundary:** explicit build prerequisites; current-source rebuilt actual exported SDK scenario; reconciled current reset fixture; distinct safe refusal of unsupported/malformed evidence; provider/deletion retry and exact-reference protection; local deletion distinct from remote revocation; affected scenarios and neighboring safety cases executed and recorded. Six owner-boundary selections plus seven native safety tests passed; all five isolated owner-composition children also passed. No assertion, timeout or compiler failure occurred in this follow-up; the incorrect zero-test selection is recorded above.

No claim that these serial focused runs qualify the default-concurrency matrix, coverage/CRAP, full OpenAPI regeneration, strict Clippy, real OS providers/trust, GUI/WAN or other tickets' readiness/timeouts. Those broader gates remain explicitly outside this authorized pass. No map or other ticket edits; no historical migration/compatibility support.
