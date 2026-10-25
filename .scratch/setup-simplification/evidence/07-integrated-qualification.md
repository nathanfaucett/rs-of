# Ticket 07: integrated setup qualification

## Disposition

**Incomplete qualification; ticket 07 remains open.** Tickets 01–06 are integrated, but the full HTTPS interruption/approval/retry flow, feature matrix and CRAP result are not reliably qualified. This report is not feature acceptance or a production certification. Do not promote the spec to resolved from the frontend/native passes alone.

- Worktree `/home/nathan/code/rust/aicacia/of-setup-07`, branch `setup-simplification-07`, base/integration `83912fa3a`. Claimed before changes; read AGENTS, tracker, spec, glossary map/shared glossary, all 01–06 receipts and the integration receipt.
- Medium coordination complexity, high readiness/security risk, high priority. Used TDD, ponytail and both Svelte skills. Only spec-authorized mounted-page, public HTTPS lifecycle and isolated native/provider seams; no new framework/dependency or provisioning implementation.
- No real profile, provider or OS trust changes. Rust builds used the existing shared build cache at `/home/nathan/code/rust/aicacia/of/target`, `CARGO_BUILD_JOBS=2`, `RUST_MIN_STACK=67108864`. Mutable sibling path dependencies are not pinned by this receipt; repeated compilation is not evidence of runtime success.

## Changes and red → green

1. Added a mounted Installation regression: an unfinished Create recovers an exact already-completed operation. It initially failed because the page never requested the current onboarding observation and stayed on the old routing. Re-observe after recovered completion, as Join already does. Completed Create submission also re-observes.
2. Held the injected native observation pending. The regression then failed because the old nested `Continue to sign in` link appeared before the current selected-target/public-login observation returned. Deleted that duplicate readiness route. The sole unified sign-in branch now consumes the coherent ready observation. Two regression cases verify no link during the pending recheck, ordinary sign-in for valid current login evidence and repair/no sign-in for missing login evidence. No fresh Create, password persistence or implicit authentication is introduced.
3. Native compilation exposed the integrated stale call `rejected.observation(&root)` after removal of the Device marker argument. Updated it to the actual zero-argument contract, preserving the cancelled-publication assertion. The complete native suite subsequently passed, including its isolated protected-provider parent/child.
4. Reconciled stale current guidance in system API/design, owner local-security, worker behavior and OpenAPI caller inventory. Removed the deleted `/setup/device` caller entry and claims that native still owns Device completion. Historical receipts remain historical. Operations/native handoff link to this current, limited qualification.

## Reproduction

Commands below are relative to this ticket worktree, not the main working tree. Frontend commands run in `apps/app`.

Prerequisites: root `pnpm install --offline --frozen-lockfile`, `pnpm packages:build`, then app `pnpm build` (generates localization). All passed. DOM checks reuse the existing environment injection:

`RESIDENCY_JSDOM_MODULE=/home/nathan/code/rust/aicacia/of-setup-04/node_modules/.cache/source-approval-test-tools/node_modules/jsdom`

That is existing disposable jsdom tooling, not a new workspace dependency. Reproduction elsewhere can point the same variable at an installed jsdom module. Generated tracked SDK/browser/bin artifacts and ticket-worktree Cargo.lock refresh are not delivery changes.

| Command | Actual outcome |
| --- | --- |
| `pnpm exec vitest run --config vitest.residency.config.ts --maxWorkers 1 --testTimeout 20000` | **34 passed / 5 files** on final frontend code: 9 independent/bundled/startup/Create cases, 9 Join, 6 source approval, 7 maintenance, 3 Residency. |
| `pnpm exec vitest run --config vitest.auth.config.ts --maxWorkers 1 --no-file-parallelism --testTimeout 20000` | **180 passed / 7 files**, repeated after final changes. Includes 28 setup-page, 39 auth-layout, 58 setup-client, 27 owner-client, 16 independent state, 7 localhost target and 5 Storage socket cases. |
| `pnpm check` | **0 errors / 0 warnings**, including final test fixtures. |
| `pnpm build` | Passed after the readiness implementation fix; static app generated. |
| `pnpm dlx @sveltejs/mcp svelte-autofixer ./src/routes/setup/installation/+page.svelte` | No issues. Existing timer/effect suggestions retained: polling requires asynchronous lifecycle cleanup, not a pure derived value. |
| `cargo fmt --all -- --check`; `git diff --check` | Passed; rerun on delivery. |
| `cargo test -p api` | **40 passed, 2 deliberately isolated child ignores; compile-fail doc test passed**. Parent-launched installer/readiness children passed. |
| `cargo test -p lidp --lib -- --test-threads=1` | **21 passed, 1 deliberately isolated child ignore**; provider parent launches child and child passed. No filter/skips added. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Failed: **93 existing generated `of-idp-client` errors**, including `empty_docs`/`derivable_impls`; no generated SDK lint rewrite. |
| `cargo clippy -p api -p lidp --all-targets --no-deps -- -D warnings` | Passed after native stale-call fix. This is scoped qualification, not a workspace Clippy pass. |
| `node --experimental-strip-types --test apps/app/src/lib/common/state/setupApi.test.mjs` (root) | **9 passed / 15 failed** with the retained standalone fixture/error-contract failures. Prior ticket 01 independently reproduced 15 baseline failures; this receipt does not rerun that original-revision comparison. Not a green frontend-wide result. |
| `cargo test -p unified-server --test setup_host` | Attempt timed out compiling its feature graph inside a 600-second combined API/setup/native invocation, after API passed; native command in that invocation was not reached. No setup_host pass claimed. |
| `cargo hack test --feature-powerset --all-targets` | **600000 ms timeout**, repeated after final Rust fix. First root package/all-features run (1/10), compiling/linking near 599/600; no completed feature matrix. Earlier archive failure in ticket01 is historical, not this run’s observed cause. |
| `just crap` | **600000 ms timeout**, repeated after final Rust fix; final coverage compilation near 597/982. No coverage completion or CRAP score. |

Several native compilation attempts also exhausted 600 seconds before reaching the stale-call error; later actual native execution passed. Tool timeouts are not tests passing, and commands later in a timed-out chain are not claimed executed. No test deadlines, authorization, provider behavior or readiness evidence were relaxed to obtain green checks.

## Public HTTPS Join: mixed evidence, not acceptance

Reused `crates/unified-server/src/https_bootstrap_tests.rs` / `api_join_tests.rs`; no competing fixture or test-only production bypass.

- `cargo test -p unified-server --lib production_api_join_ -- --nocapture`: built and ran all three parents within 600 seconds. **All three failed** at ordinary OAuth HTTPS timeout (`https_bootstrap_tests.rs:911`) before their Join assertions.
- The initially built test executable, selected only for `production_api_join_completion_and_offline_reopen --nocapture --test-threads=1`, **passed** in 91.22 seconds. This is a real isolated HTTPS Join/source-offline reopen execution, not GUI evidence.
- `cargo test -p unified-server --lib production_api_join_ -- --nocapture --test-threads=1`: rebuilt, then finalization and offline-reopen parents/children passed (110.51 / 90.74 seconds for children). The overall 600-second bound expired after interruption started, so **the command did not pass**.
- Reused that freshly built executable directly, `/home/nathan/code/rust/aicacia/of/target/debug/deps/unified_server-f38d95073d23ab7e production_api_join_ --nocapture --test-threads=1`, to separate execution from compilation. **0 passed / 3 failed** in 212.06 seconds: finalization timed out during ordinary authenticated Storage (`api_join_tests.rs:633`); offline and interruption timed out on ordinary authorized source approval (`api_join_tests.rs:425`).

The varying timeout points suggest timing sensitivity but do not establish its root cause or an unrelated baseline. Individual passes are retained without hiding subsequent failures. Interruption/exact retry end-to-end remains unqualified; do not increase deadlines, weaken assertions or call it resolved merely because one offline execution passed.

## Coverage and call-graph audit

| Requirement | Evidence and boundary |
| --- | --- |
| Default Create/Join, required credentials, default name, ordinary sign-in | Mounted Installation/Join and loader/auth-route tests pass. New completed-Create recovery cases prove the current-login gate. No automatic session or Device dismissal. |
| Join request, source acceptance, exact approval/rejection, readiness | Join/source-approval mounted tests use real HTTP/native adapter boundaries; owner authorization/client tests pass. Full public HTTPS result is mixed as above, not reliably qualified. |
| Durable accepted work, restart, exact retry, transient credential re-entry | Mounted Create lost-response/unmount/recovery and Join exact recovery/timeout/cancel tests pass; native provider Create/reopen passes. Public HTTPS interrupted Join remains unqualified. |
| Visible startup, existing trust, refusal, prerequisites, contention | Mounted startup/consent/refusal/retry plus native actual TLS/injected trust, reset/close/publication fencing pass. Native window construction precedes async spawn by inspection; no physically drawn desktop window certified. |
| Advanced remote/browser/independent owners | Rendered remote selection despite failed bundled host, no native browser calls, no bundled capability forwarding, owner-specific codes and correlated operation/readiness state tests pass. Not a real remote deployment acceptance. |
| Optional data selection and maintenance | Existing Residency/owner-client restriction/transfer cases and seven mounted maintenance cases pass; native provider refusal, exact-reference conflict, data-only retry and issuer/per-owner revocation acknowledgements pass. No restriction or resource authority changed. |
| Negative/security cases | Retained admission/Origin, malformed owner/login evidence, secret disclosure, target changes and lost TLS/identity/provider evidence tests remain. API/native suites pass at their isolated seams; failed HTTPS cases do not certify every wrong-peer/issuer/approval assertion end-to-end. |

Trace: setup entry/sign-in/auth layout → `observeOnboarding`; Installation → shared `WebSetupCreate` / dedicated `JoinSetup`; source Devices → `SourceApproval` → ordinary authorized owner clients; deferred Devices → existing policy/Residency APIs; `/maintenance` → retained recovery UI → existing serialized native lifecycle/reset/revocation. Native command registration/HostObservation no longer exposes Device completion. Search finds no production `/setup/device`, `complete_device_setup`, `device_setup_required` or marker implementation. Test fixtures intentionally injecting the old extra field and the native absence assertion are negative coverage, not active workflow state. Shared worker durable completion/admission records are necessary and not obsolete dismissal markers. The duplicate nested sign-in branch was the remaining superseded readiness path removed here.

## Coordination with system-test effort

Read `tests/system/README.md`, root `tests/system_create.rs` and `examples/system-test-service.rs`. The existing opt-in process suite owns unified/independent Create/use, retained restart, authorized Join/source-offline reopen and permission/resource denials. Its private-provider launcher, isolated trust commands and generated-client scenarios are reused by reference, not duplicated or moved. The referenced `.scratch/deployment-independent-system-tests` reports are absent from both current worktrees; do not invent their results. Those opt-in process scenarios were **not run** here: required launcher/identity-database builds and discovery networking are additional qualification. Existing private HTTPS corruption/interruption tests are not deleted because they cover distinct retained-state failures. No claim that backend tests qualify GUI interactions.

## Remaining work and safety

1. Stabilize/reproduce the HTTPS request timeouts, then obtain a complete passing interruption/finalization/offline run and qualify the existing opt-in process scenarios as appropriate.
2. Complete the feature matrix and CRAP with a stable build/dependency snapshot and adequate resources. Scoped Clippy success does not waive the generated workspace lint failures or standalone setup HTTP failures.
3. Real OS trust/elevation, physical/WAN pairing, real providers/profiles, other platforms, native GUI rendering and production deployment remain explicitly outside automated injected same-host evidence.

Main worktree `Cargo.lock` was read-only throughout and retained SHA-256 `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`; no main source edits, branch switch or commits. Ticket-worktree generated lock/SDK changes are restored, not delivered. Owned test/build commands are bounded; no running owned Cargo/test process remained at the cleanup inspection.
