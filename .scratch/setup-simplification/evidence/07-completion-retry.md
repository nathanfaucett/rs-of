# Ticket 07 completion retry — 2026-10-28

## Disposition

**Implemented two demonstrated lifecycle defects; integrated qualification remains incomplete.** Ticket 07, its two unchecked integrated Join/durability criteria, and the spec remain open. A single passing operation parent from the preceding attempt did not qualify the workflow.

Branch `integration/setup-07-completion` starts at `main` `31ced611f4afa08940fcf6c8247ac966bcf81de2`. No real profiles, provider stores or OS trust stores were modified. Existing worktrees remain on disk. Four accidentally committed worktree gitlinks from the preceding attempt are removed from the index; their repositories are not deleted. Existing sibling work and root lockfile are preserved.

## Changes and causal regressions

### HTTPS verification cancellation

`crates/api/src/setup_server.rs` previously awaited the OPTIONS verification request without selecting cancellation or listener completion. With the existing 30-second request budget, cancellation could wait behind verification before even entering the joined shutdown path.

Verification now races cancellation and listener termination. Cancellation suppresses announcements and enters the existing cleanup path; listener/connection destruction still precedes owner-guard release.

The gated HTTPS OPTIONS regression in `setup_server_teardown_tests.rs` uses isolated TLS material and a test-CA client. It verifies owner exclusion while the request is outstanding, cancellation overriding a late failed verification response, joined cleanup, and immediate reopen with unchanged admission and CA bytes. Temporarily restoring the direct verification await produces **1 failed** (`setup HTTPS status verification failed`); restoring the fix passes in the final API suite. This is a deterministic control-path regression, not proof that this defect caused the historical Join failures.

### Registration lifecycle mutex

`crates/api/src/independent_exchange.rs::register` held the lifecycle mutex across asynchronous registrar provisioning. Status, cancellation and finalization use the same mutex.

The handler now consumes the exact challenge and captures the active cancellation token under the mutex, releases it during registrar work, then reacquires it and checks the original cancellation token, current active operation, exact intent, shutdown and admission before confirming a receipt. Already committed domain provisioning is not rolled back or misrepresented as undone.

The new `independent_exchange_lifecycle_tests.rs` gates the registrar. Before the fix, status cannot complete while it is gated (**red: 1 failed**). After the fix, status and authorized exact cancellation complete, late registrar success is forbidden, no receipt is confirmed, cancelled finalization fails and admission is not retired (**focused green: 2 passed**). Full final API tests also pass.

### Restore timeout coverage and enforce real test budgets

The previous commit changed the delayed-authority regression to expect 503, removing client-timeout coverage. `tests/owner_composition_tests/operation.rs` now gives only the intentionally stalled status request a one-second request-local timeout. The production default remains 30 seconds. The test still asserts timeout, listener survival after the delayed remote failure, current readiness denial, same-host recovery and retained identity. Its three-second initial-router polling budget now encloses the asynchronous request rather than checking the clock only after failures.

`tests/owner_composition_tests/create.rs::wait` now encloses all owner observations and body reads in the existing shared 60-second budget. This prevents late successful responses from silently extending qualification. No readiness, authorization, source approval, identity or provider assertion is removed. The unchanged 90/180-second process watchdogs remain in place.

## Fixed inputs and reproduction

Commands ran from `/home/nathan/code/rust/aicacia/of` with:

```sh
CARGO_BUILD_JOBS=1
RUST_MIN_STACK=67108864
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target
```

Sibling heads throughout recorded runs:

- `ofdb`: `6286ca900f7a3fe73fa00e233656afab48d1fb81`
- `ofnet`: `dbea925a0a3f99053fffc02618baccc2a1cbea9f`
- `offs`: `df4c696a910296635473ec296d9235acbaa60d9b`

Root `Cargo.lock` SHA-256: `a39e0f5211fb541b56d9e6cd8c498b2e9ce330c8ca2281c80cd4205ef5b532d1`.

Local raw logs, argv, environment, revision/hash receipts and resource samples are under `target/setup07-completion/`; these are disposable local artifacts, not committed evidence dependencies. Ordinary runs clear timing/secret-lock diagnostics and test-concurrency overrides. The separately labeled diagnostic enables the existing `OF_SETUP_TIMEOUT_TIMING` and `OF_SETUP_SECRET_LOCK_DIAGNOSTIC` switches; it is not pooled with ordinary qualification.

## Actual outcomes

| Command / cell | Result |
| --- | --- |
| `cargo test -p api --features test-support --lib setup_server:: -- --nocapture` | 3 passed / 1 isolated child ignore; parent-launched delayed-readiness child passed. Before registration changes. |
| `cargo test -p api --lib independent_exchange:: -- --nocapture` | Agent-executed red 1 failed before lock fix; green 2 passed after it. |
| `cargo test -p api --features test-support --lib cancellation_interrupts_pending_https_verification_before_owner_reopen -- --nocapture` with direct-await implementation restored temporarily | Deterministic red: 1 failed; fixed source restored immediately afterward. |
| `cargo test -p api --features test-support --lib` on final implementation | **44 passed / 2 isolated child ignores**, 7.26s; parents launched their installer/readiness children. |
| `cargo clippy -p api --all-targets --all-features --no-deps -- -D warnings` | **Passed**; scoped, not workspace Clippy. |
| `cargo test -p unified-server --lib production_api_join_ -- --test-threads=1` | **3 passed**, 175.14s test execution (804.478s including compilation). Finalization, completed source-offline reopen, interruption/exact retained retry. |
| `cargo test -p idp-unified --all-features --test owner_boundaries production_independent_signer_free_operation_and_delayed_authority_reopen -- --test-threads=1` | **1 parent passed**, child passed in 40.60s; final timeout and initial-router-budget assertions included. |
| `cargo test -p idp-unified --all-features --test owner_boundaries production_independent_join_and_source_offline_reopen -- --test-threads=1` | **1 parent passed**, child passed in 54.30s. Existing exact rejection/cancellation/restart/retry/source-offline scenario reused. |
| `node --experimental-strip-types --test apps/app/src/lib/common/state/setupApi.test.mjs` | **24 passed**, 0 failed. No Svelte source changed. |
| `cargo fmt --all -- --check`; `git diff --check` | Passed. |
| Default-concurrency owner suite, before registrar lock fix | **39 passed / 5 failed / 6 ignored**, 255.82s. Supervisor was killed by the tool's 600s invocation limit after compilation; the owned workload survived and completed. A follow-up monitor checked RAM/PSI while awaiting completion. No complete supervisor receipt or inherited exit status is claimed for this cell; the log records the actual failed libtest result. |
| `cargo test -p idp-unified --all-features --test owner_boundaries`, after registrar lock fix (`owners-after-lock`) | **39 passed / 5 failed / 6 ignored**, 250.32s libtest / 329.505s supervised, exit 101. |
| Same full workload, separately timed diagnostic (`diagnostic`) | **40 passed / 4 failed / 6 ignored**, 242.20s libtest / 244.126s supervised, exit 101. |

The ordinary full runs include the restored client-timeout assertion and shared hard readiness deadline, but precede the final additional three-second enclosing router-poll timeout. The final focused operation parent includes that additional assertion. Do not describe a failed preceding workload as a complete final-source acceptance run.

## Exact remaining failures and localization

After the registrar fix, the ordinary full workload still fails:

- Both independent Join parents: 60-second readiness observation budget expires.
- Both independent Create parents: 180-second child watchdog expires.
- Independent signer-free operation parent: 90-second child watchdog expires.

The separate diagnostic passes the ordinary Create parent but retains the reset Create, both Joins and operation failures. This variability is not a latency improvement measurement or a baseline attribution.

Correlated diagnostic records establish distinct Join boundaries:

- PID `314199`: final readiness wait begins at Unix-ms `1793184657050`; a current IdP check completes at `1793184718918`, after the wait budget has expired. The check itself reports 3253ms. Lifecycle had remained `joining` before reaching `ready`.
- PID `314219`: final readiness wait begins at `1793184657938`; IdP matches after **59968ms**, then Device still reports `joining` at `1793184717906`. The shared deadline expires before all three owners are ready.

Thus a 30-second individual request budget is not the missing end-to-end performance fix. Releasing the registrar lifecycle lock fixes control responsiveness but does not eliminate the measured remaining Join preparation/finalization time.

The unified isolated passes report public-bootstrap durations around 30–31 seconds. Source tracing found a 30-second retry deadline checked after failed attempts, **not a fixed successful-path sleep**. The timer includes approval wait, connection, sequential IdP/IdentityPolicy/Device history imports and final public-readiness checks. Isolated test endpoints use in-memory address lookup with relays disabled. No current per-stage profile proves the dominant cost; do not remove validation or label it discovery-only/CPU-only from this outer timer.

## Resource and review boundaries

The completed supervised cells did not hit the 2GiB available-RAM, 30GiB disk, sustained memory-PSI, or 7200-second command guards. The diagnostic's peak sampled aggregate owned RSS was **1.09GiB**, including Cargo/parent/children; it was essentially execution-only. Build-plus-test cells reached approximately **4.9–5.0GiB aggregate owned RSS** during their complete invocation; do not call that application runtime memory or claim every process used little memory. No unrelated processes were terminated.

`change-review` was unavailable in the skill registry. An independent read-only agent review of the lifecycle fixes and regressions found no actionable finding; it did not execute tests or certify completion. No feature matrix, workspace Clippy, fresh coverage/CRAP, opt-in process system suite, packaged GUI, real provider/OS trust or WAN qualification is claimed in this retry. Ticket 11's broad gates remain separate and incomplete; the failed concurrent prerequisite is not waived.

Next causal measurement: distinguish pairing/approval, connection, per-context schema/admission/import, relationship registration and lifecycle finalization on the failing default-concurrency workload. Existing outer duration observations do not justify raising deadlines, cached authorization, weakened import checks or a speculative SQL rewrite.
