# Ticket03: cross-layout lifecycle and filesystem qualification

## Scope and branch

- Branch: `unified-owner-lifecycle-03`.
- Worktree: `/home/nathan/code/rust/aicacia/of-cross-layout-03`.
- Integration base: `6b6480ab53aa9f249cacfebe138473d778aaf8bd` (`unified-owner-lifecycle`).
- Claimed ticket03 before implementation. Skills loaded: `tdd`, `ponytail`.
- Approved seams come from the spec and user request: existing production-process
  deployment scenarios, generated owner/storage clients and typed Storage protocol,
  focused live reconciliation regression, unchanged offs in-process tests.
- Complexity moderate; risk high around listener startup, trust, retained owner
  operations and content/protocol wiring; priority qualification prerequisite for 04.
- No production semantics, dependencies, offs source/tests or worktrees 01/02 changed.
  `just crap` generated six tracked `node_modules/.bin` path changes in this new
  worktree; only those agent-generated changes were restored. Ticket02 user launcher
  changes were not touched.

## Implementation and coverage

`tests/system/create.rs` now launches Unified through ordinary production config
and verified HTTPS with one CA and three owner capabilities. Both layouts share
`owner_create`, owner readiness/public operation checks, exact/conflicting retries,
ordinary Management authorization, retained restart and Join scenario bodies.
There is no old aggregate setup path or host-to-owner compatibility adapter.

`tests/system/recovery.rs` consumes only owner-local Join recovery/cancellation;
its deleted host variant and topology-specific broker calls are gone. Current
owner Device admission/rejection uses the generated independent-join API in both
layouts. Stale offers return the owner contract 409; the old deleted Unified broker
404 is not preserved as an alias. Cancellation, interruption, exact retry, rejection,
learned grant denial, approval and source-offline reopen assertions are retained.

`tests/system/filesystem.rs` adds shared generated resource/residency operations
and typed production WebSocket write/read/delete/list. Its verified TLS connection
builder is extracted from and reused by `authorization.rs`, rather than adding a
second transport harness. A retained filesystem keeps exact bytes and file deletion
across restart. A populated disposable filesystem is tombstoned; generated detail
returns 404 and catalog listing excludes it both before and after restart.
An empty disposable resource changes Passthrough/Full and actual writes demonstrate
read-only/restored-write effects. This is not cross-node content synchronization.

All existing nine process scenario entries remain opt-in and serial, with isolated
roots, mock credential/OS trust providers, explicit loopback bases and bounded
HTTP/startup/readiness/recovery/protocol waits. The ordinary private-root regression
remains separate. `tests/system/README.md` describes the current suite and limits,
replacing obsolete broker/removed-test instructions.

## Live-reconciliation root cause: evidence, red, green

The merger reported ConnectionRefused at the target IdP Join (`live_reconciliation.rs`
original line 325). `SetupHost::prepare` prepares state but does not bind a listener.
`SetupHost::serve` binds inside the future; `tokio::spawn` does not guarantee that
future has been polled before the next request. Source startup had a best-effort
wait; target startup had none. Retained readiness polling also immediately panicked
on a startup connection refusal instead of polling.

1. Added a deterministic 200 ms delay inside the target serving task, with the
   existing immediate Join unchanged. The focused test failed in 23.77 s with the
   exact IdP `/setup/owner/join` ConnectionRefused (OS error 111), before the target
   listener bound. This reproduces the reported failure rather than assuming it.
2. Replaced the source best-effort wait and shared readiness helper with bounded
   public-status polling. Only connect/timeouts are retried; HTTP status and decode
   failures still fail. Per-request timeout is 2 s, total polling deadline 60 s.
3. Target now waits for its public status listener and asserts not-ready before
   submitting Join. This deliberately waits for availability, not setup completion
   (which cannot happen before admission). The startup delay remains as regression
   coverage. No mutation retry, fixed post-spawn sleep or production change is needed.
4. The delayed-start test passed in 57.33 s. Final Unified all-feature run passed
   composition and the delayed-start live Create/Join/source-offline reopen test,
   with the live case completing in 58.44 s.

Ticket02 earlier timeout results remain timeout results. This report establishes
new completed runtime evidence; it does not retroactively declare ticket02 green.

## Filesystem red/green and fixture finding

- Before migrating the harness, `unified_create_and_use` failed at the obsolete
  aggregate setup startup wait in 30.05 s. After migration the same entry passed
  in 21.85 s; shared Unified Join also passed in 76.11 s before filesystem expansion.
- Added retained protocol content assertion before adding its writer. Unified
  retained restart failed with typed `Error { code: OperationFailed }` instead of
  the expected literal bytes. Adding the actual production protocol write made
  it pass in 37.12 s. Catalog success alone cannot satisfy this suite: the content
  connection and exact bytes must also work before and after retained restart.
- Initial residency characterization attempted to restore a populated, evicted
  single-node resource from Passthrough to Full. It returned HTTP 500; private
  service diagnostics identified `Storage Residency owner update failed: content
  is unavailable`. The existing Full transition fetches evicted content and there
  is no Full peer in this single-node scenario. This was a fixture error, not
  evidence for changing offs or Storage semantics. Residency now uses an empty
  disposable filesystem, proves actual write denial/restoration, then populates
  and deletes it. The independent retained scenario passed in 95.52 s and Unified
  retained scenario in 85.30 s during focused development.

## Exact validation commands and results

Unless a different directory is given, every command below ran in the ticket03
worktree via `env -C /home/nathan/code/rust/aicacia/of-cross-layout-03`. No shared
Cargo target override or changes in worktrees 01/02 were used.

### Passed

- `cargo build -j 8 -p idp-unified --example system-test-service -p idp-server --bin identity-database`
  — built the real service launcher and remote identity database (2m02s).
- `cargo test -j 8 -p idp-unified --test system_create unified_create_and_use -- --ignored --exact --test-threads=1 --nocapture`
  — original startup red, transient compile errors while migrating removed APIs,
  then green as described above.
- `cargo test -j 8 -p idp-unified --test system_create unified_join_and_source_offline_reopen -- --ignored --exact --test-threads=1 --nocapture`
  — passed after lifecycle migration, 76.11 s.
- `cargo test -j 8 -p idp-unified --test system_create unified_retained_restart -- --ignored --exact --test-threads=1 --nocapture`
  — content red/green and residency fixture finding above, then passed.
- `cargo test -j 8 -p idp-unified --test system_create independent_retained_restart -- --ignored --exact --test-threads=1 --nocapture`
  — passed, 95.52 s.
- **Final entire process suite:**
  `cargo test -j 8 -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture`
  — **9 passed, 0 failed, 1 ordinary root test filtered out, 915.46 s**. Includes
  Create, restart, Join/source-offline reopen and permission/resource denial for
  both layouts plus independent local-owner Create/restart. Every shared use
  includes the new filesystem body, including source-offline retained content.
- `target/debug/deps/system_create-f7934f13c9d5f765 --exact create::root_is_private --test-threads=1 --nocapture`
  — passed ordinary regression and isolated permissive-umask child; invoked the
  already-built binary after the unrelated sibling manifest blocker appeared.
- `cargo test -j 8 -p unified-server --all-features --test live_reconciliation -- --nocapture`
  — deterministic startup red followed by green, as above.
- **Final Unified all-feature runtime:**
  `cargo test -j 8 -p unified-server --all-features -- --test-threads=1 --nocapture`
  — **2 passed**, composition and live reconciliation; live 58.44 s. Unit/bin/doc
  targets contained zero tests and are not additional coverage.
- `cargo clippy -j 8 -p idp-unified --test system_create --no-deps -- -D warnings`
  — passed against the final Rust sources, including new filesystem assertions.
- In sibling `/home/nathan/code/rust/aicacia/offs`:
  `cargo hack test --feature-powerset --workspace --all-targets`
  — **passed**, unchanged in-process filesystem tests. This completed before
  concurrent unrelated manifest edits described below; no claim is made that
  its later uncommitted manifest is buildable.
- `cargo fmt --all` — applied successfully before final runtime runs.
- `rustfmt --edition 2024 --check crates/unified-server/tests/live_reconciliation.rs tests/system/create.rs tests/system/authorization.rs tests/system/filesystem.rs tests/system/recovery.rs`
  — passed final targeted formatting without loading the broken sibling manifest.
- `git --no-pager diff --check` — passed.
- Source audit (Python regex over `tests/system/*.rs`) found no raw HTTP request
  builders (`.get/.post/.put/.delete/.request`) or deleted `setup_api as setup`
  imports. `git --no-pager -C ../offs diff --name-only -- crates/file-system/src crates/file-system/tests`
  returned no filesystem source/test changes. These are audits, not runtime tests.

### Timeouts and broader gates: not green

- Initial fresh-target runs of
  `cargo test -j 8 -p unified-server --all-features --test live_reconciliation -- --nocapture`
  hit two 240-second compilation limits, then a third 240-second combined
  compile/runtime limit (compilation completed in 2m39s; runtime still active).
  No passing runtime claim is made for these attempts. Later focused and full
  all-feature runs completed as recorded above.
- The initial combined launcher build plus focused system test command hit a
  240-second limit after its 2m02s build and 1m53s test compilation, during scenario
  execution. The separate rerun produced the obsolete-harness startup red above.
- `timeout --kill-after=10s 300s cargo hack test --feature-powerset --workspace --all-targets -j 8`
  — failed immediately at Cargo metadata: duplicate target-dependencies table in
  sibling `offs/crates/file-system/Cargo.toml`, initially line 75.
- `cargo hack test --feature-powerset --all-targets -j 8`
  — failed at the same metadata blocker (then line 70).
- `timeout --kill-after=10s 300s just crap`
  — SDK prerequisites completed; coverage/CRAP failed at the same sibling manifest
  duplicate-key error. No coverage or CRAP score was produced.

The sibling offs manifest and `.scratch/notes` had concurrent uncommitted changes,
not authored by ticket03; neither was overwritten, staged or fixed. The duplicate
was still present on the final root-powerset attempt. Full of feature-powerset and
CRAP qualification remain blocked, not baseline-passed or timeout-passed. Rerun
these gates once that external manifest edit is complete. No WAN, real credential
provider, OS trust installation, GUI, non-Linux, cross-node filesystem sync or
post-Join production-process reconciliation qualification is claimed.

## Integration handoff

The implementation is committed on ticket03 only. Final integration-tip merge
result is recorded below after the commit; integration itself is not merged by
this agent. Ticket04 owns the remaining frontend/operator/global-doc migration.

- Implementation commit: `22b363d02` (`Qualify owner lifecycle and filesystem across production layouts`).
- After that commit, ran `GIT_EDITOR=true git merge --no-edit unified-owner-lifecycle` in ticket03: **Already up to date.** Integration tip was still exactly `6b6480ab53aa9f249cacfebe138473d778aaf8bd`; it is an ancestor of ticket03. No integration branch or other worktree was merged/modified.
- Post-merge `git --no-optional-locks status --short` was empty and `git --no-pager diff HEAD --check` passed. This final handoff entry is committed separately; no Rust changes followed the successful full scenario run.
