# Final review: four owner-lifecycle findings

## Scope and isolation

- Implementation branch: `unified-owner-lifecycle-review-fixes`, using the existing `of` worktree, branched from integration `unified-owner-lifecycle` at exactly `58f4599da11b058edf447886f048f3ca9322fa99`.
- These are corrections to the existing spec/tickets, not new requirements or tickets. No broker, migration, old-version compatibility path, dependency, or parallel lifecycle was added.
- Loaded `tdd` and `ponytail`. Changes and tests are Rust-only; no Svelte components/modules were analyzed or edited, so Svelte skills were not needed.
- Seams: stopped desktop/owner reset, fresh composed Management HTTP metadata, and the existing CLI submission boundary over pinned HTTPS. The composition suite still has exactly one test; its fresh-host assertions were extended.
- Complexity: moderate (durable acknowledgment across two journals). Risk: high for reset recovery and exact-operation/capability handling; lower for state-free metadata. Priority: the two P1 blockers, then the two P2 findings.
- The sibling offs manifest and untracked `.scratch/notes/` were inspected but never edited, staged, restored, or committed. Its manifest SHA-256 at final validation was `b7ec6a37f410d393fe0594e4cf13d8763587fd44f0de714b9746bfc3027a0a88`. No other worktree was modified.

## Findings, evidence, and fixes

### P1: owner reset erased recovery evidence before desktop acknowledgment

**Root cause:** `setup_reset::run` awaited each owner's reset, then published that owner's completion in `reset-state.json`. `IndependentReset::finish` had already removed the domain, setup directory, and reset journal before returning. A failed desktop publish or interruption left the desktop pending, but neither the original setup directory nor the reset journal required by `IndependentReset::open` remained.

**Fix:** `IndependentReset::finish` now takes an explicit acknowledgment callback. It durably removes owner data/setup state, retains the exact Cleanup journal, invokes acknowledgment, and only then deletes and syncs the reset journal. IdP, Device, and Storage reset entry points pass this callback through. Desktop callbacks durably publish per-owner completion. If desktop completion already exists but the corresponding owner reset journal remains, desktop retries that owner's exact cleanup/acknowledgment rather than skipping its recovery evidence. Standalone CLI callers explicitly acknowledge locally with `|| Ok(())`; all existing callers and fixtures were updated. There is no old-signature wrapper or new journal format.

**Regression:** `interrupted_owner_completion_resumes_before_and_after_desktop_acknowledgment` exercises all three owners with acknowledgment failure both before desktop completion is published and after it is durably published (six interruption cases). It checks pending reset/startup refusal, successful exact reset resumption, and fresh setup eligibility afterward. Injection occurs at the acknowledgment I/O boundary after real owner cleanup; it is not a claim of OS process-kill or power-loss testing. The existing public owner-contract test `resetting_one_composed_owner_preserves_the_shared_authority_and_other_owner` also now injects acknowledgment failure, requires startup to remain fenced, and reopens completion evidence before acknowledging; it retains its live sibling and shared-authority assertions. Existing stopped-worker, inventory, and trust-removal tests remain.

### P1: fresh composed Management did not publish metadata

**Root cause:** the composed Management fallback returned 503 for every request until Device published its serving state, including `/management/version`, which connection validation needs before provisioning. The Management crate already supplies a state-free `public_metadata_router`.

**Fix:** the host dispatches fresh Management requests to that existing metadata router, with a 503 fallback for runtime routes. Once the runtime router is published it receives requests unchanged, with its existing prefix. Metadata opens no database, grants no setup capability, and does not publish readiness or start Management early.

**Regression:** the existing single composition test now requests a fresh host's Management version and OpenAPI metadata, verifies Management identity/canonical owner base, and requires Management health to remain 503. Existing prefix, setup-path, and previous-layout refusal assertions are retained.

### P2: CLI exact retry failed at an already retired owner

**Root cause:** `submit` required `SetupAdmission::protected_code` to return a live code for every owner. A completed first owner therefore prevented subsequent incomplete owners from receiving the exact retry.

**Fix:** retired owners are observed without an Authorization header or mutation. The CLI requires current ready status for exactly that owner and public metadata containing exactly the requested operation and owner before continuing. Equality covers purpose, nonce, administrator, node, issuer, all owner bases, and peer. Missing/unready/conflicting evidence fails closed. A 401 caused by retirement racing the POST uses the same read-only completion check. No capability is reopened/reactivated, and later incomplete owners still receive only their own existing capabilities; password remains IdP-only.

**Regression:** `exact_retry_observes_retired_owner_and_submits_only_incomplete_owners` drives the CLI submission boundary against isolated pinned HTTPS and real protected admission files. It covers an already retired first owner, a retirement-during-request 401 race, conflicts in every operation field, wrong public/status owner, not-ready status, and unavailable public metadata (15 cases). It checks which owners received POSTs, scoped Authorization, password exclusion from Device/Storage, and that the first owner's capability remains retired. These are CLI HTTP-consumer fixtures, not duplicate owner-worker provisioning tests.

### P2: desktop full reset deleted shared TLS and listener identity

**Root cause:** after the three owner-local resets, desktop called `remove_dir_all(node)`, deleting shared `endpoint.tls` and `listen.addr` even though the documented reset boundary preserves them. Owner reset already handles shared TLS correctly.

**Fix:** remove the broad root deletion. Owner-local reset remains responsible for owner data, setup admission, credentials, and journals; the shared TLS root and retained listener address are left intact. No allowlist-based recursive deletion or compatibility shim was added.

**Regression:** `reset_uses_the_stopped_owner_reset_boundaries` now compares the SHA-256 CA certificate fingerprint, exact bytes of all five retained TLS files, and listener address before reset and after preparing fresh owners, and checks that reset is no longer pending. No real OS trust installation/removal is needed for this shared-TLS reset test.

## TDD and validation limits

The initial finding regressions were written before their corresponding implementation changes; the existing owner-contract test and additional CLI exactness/race cases were extended during final call-site review. Initial composition, desktop reset, and CLI retry test commands all stopped at the unrelated sibling manifest error, so there was **no executable red/green cycle**. Findings are grounded in the inspected call paths; new regressions have **not compiled or run**. Formatting/parser checks are not a substitute for compilation. Empty editor diagnostics are not counted as Rust validation.

### Passing checks

From `of`:

```sh
rustfmt --edition 2024 --check apps/app/src-tauri/src/setup_reset.rs apps/app/src-tauri/src/setup_reset_tests.rs crates/api/src/independent_reset.rs crates/device-server/src/cli/run.rs crates/device-server/src/independent_reset.rs crates/idp-server/src/cli/run.rs crates/idp-server/src/independent_reset.rs crates/storage-server/src/cli/run.rs crates/storage-server/src/independent_reset.rs crates/unified-server/src/cli.rs crates/unified-server/src/cli_tests.rs crates/unified-server/src/setup_host.rs crates/unified-server/tests/composition.rs tests/owner_boundaries/device_authority_tests/remote.rs tests/owner_composition_tests/remote_setup.rs tests/owner_composition_tests/reset.rs
git --no-pager diff --check
```

Both passed. A final `rustfmt --edition 2024 --check crates/api/src/independent_setup_tests.rs` also passed after updating the remaining direct reset API test call site. The same targeted `rustfmt` command without `--check` was used to format the files first. The four SDK ESM builds (`pnpm sdk:build`, invoked by `just crap`) passed; no generated contract/source changes resulted.

### Blocked checks: exact commands

Every following Cargo command failed before compilation with duplicate target-dependencies table at `../offs/crates/file-system/Cargo.toml:70:67` (or `crates/file-system/Cargo.toml:70:67` when run from offs). Cargo exited 101; cargo-hack exited 1 while invoking Cargo metadata. No test counts or successful runtime/feature combinations are claimed.

From `of`:

```sh
cargo test -p unified-server --test composition --no-default-features
cargo test -p unified-server --features cli --lib exact_retry_observes_retired_owner_and_submits_only_incomplete_owners
cargo test -p lidp setup_reset --lib
cargo test -p api independent_reset --lib
cargo test -p api resetting_one_composed_owner_preserves_the_shared_authority_and_other_owner --lib
cargo test -p idp-unified --test owner_boundaries owner_composition_tests -- --test-threads=1
cargo build -j 8 -p idp-unified --example system-test-service -p idp-server --bin identity-database
cargo test -j 8 -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture
cargo clippy -p api -p idp-server -p device-server -p storage-server -p unified-server -p lidp --all-targets --all-features --no-deps -- -D warnings
cargo hack test --feature-powerset --workspace --all-targets
just crap
```

`just crap` exited 101 after successful SDK prerequisites: `cargo llvm-cov --workspace --lcov --output-path /tmp/lcov.info` failed at metadata, so `cargo crap` did not run and no complexity score was obtained. An initial `cargo test --test owner_composition -- --test-threads=1` attempt also failed at that metadata error before target resolution; the actual `owner_boundaries owner_composition_tests` target/filter was subsequently used above.

From unchanged sibling `offs`:

```sh
cargo hack test --feature-powerset --workspace --all-targets
```

Also blocked at the duplicate table. No edits were made to get any gate past the blocker. No full GUI, browser, credential-provider, privileged trust, real crash/power-loss, filesystem cross-node sync, or other-platform qualification was performed. Rerun these checks after the sibling owner resolves the manifest.

## Original integration handoff (before independent review)

- Implementation commit: `9d3301f05c0a34f5b96f3e2d6ff7cac5fc855f68` (`Fix owner reset acknowledgment, fresh metadata, and exact CLI retries`).
- Ran `GIT_EDITOR=true git merge --no-edit unified-owner-lifecycle` on the implementation branch: **Already up to date.** Integration remains `58f4599da11b058edf447886f048f3ca9322fa99` and its ancestry check succeeded. No merge into integration was performed.
- Implementation worktree was clean after commit/merge. The sibling manifest SHA-256 was unchanged after merge. This handoff update is committed separately; no production code changed after the final formatting/blocked test checks.
- Git automatically reported background packing and its existing `gc.log`/unreachable-loose-object warning during commit. No manual GC, pruning, or removal of that log was performed.

## Independent review and integration delivery

Reviewed `3099be446` (implementation `9d3301f05`) against integration `58f4599da`, located with `git worktree list`. The fixes branch occupied the existing `of` worktree; integration was not checked out elsewhere. Read all four fixes and regression diffs, traced reset open/cleanup/reopen and desktop publication, inspected admission bind/retirement and exact operation types, Management metadata/prefix dispatch, reset call sites, native startup ownership, and ticket/spec/map status.

**Finding corrected:** `apps/app/src-tauri/src/desktop_lifecycle_tests.rs::reset_during_startup_cancels_and_joins_before_deleting_isolated_state` still asserted that `node` disappeared. That assertion necessarily fails under the intended shared TLS/listener retention behavior. Replaced it with assertions that all three owner roots/setup/reset evidence are removed, the shared CA and listener/issuer identity are unchanged, and desktop reset is no longer pending. Startup cancellation/join and listener release assertions remain. This is a focused test correction; no further production change was identified.

**Verdict:** the four implementation fixes are acceptable on independent static review, with the stale lifecycle-test assertion corrected. Call-site arities, closure ownership/borrowing, operation equality types, HTTPS fixture handlers, feature/dependency availability, and prefix flow appear plausible on inspection, but this is **not compiler evidence**. No claim that the final Rust source/tests compile or pass, and no complete validated acceptance. Reset callbacks test acknowledgment-boundary failures, not real crash/power-loss behavior; CLI fixtures do not provision real workers and primarily exercise retirement of the first owner. Broader real-provider/trust, GUI, cross-node filesystem and platform limits above still apply.

All four implementation tickets remain resolved. Ticket02/04 now link these corrections and qualification limits; map Frontier no longer points to the obsolete ticket04 handoff. Spec triage is `needs-triage`, with implementation resolved and final qualification explicitly blocked, rather than stale `ready-for-agent` or a fully validated status. Ticket01's unchecked historical deployment qualification is not silently promoted to green.

### Independent validation attempts

- Passed targeted `rustfmt --edition 2024 --check` on all 17 changed Rust files from the original fixes plus `apps/app/src-tauri/src/desktop_lifecycle_tests.rs` (18 files total).
- Passed `git --no-pager diff --check 58f4599da..HEAD` before the correction; final working-tree/commit whitespace checks also required before merge.
- The following commands were actually retried from `of`; each Cargo command stopped before compilation at the unchanged `../offs/crates/file-system/Cargo.toml:70:67` duplicate table (exit 101), and cargo-hack stopped in metadata (exit 1):

```sh
cargo test -p unified-server --features cli --lib exact_retry_observes_retired_owner_and_submits_only_incomplete_owners
cargo test -p lidp --lib reset_during_startup_cancels_and_joins_before_deleting_isolated_state
cargo test -p lidp setup_reset --lib
cargo test -p api independent_reset --lib
cargo test -p api resetting_one_composed_owner_preserves_the_shared_authority_and_other_owner --lib
cargo test -p unified-server --test composition --no-default-features
cargo clippy -p api -p idp-server -p device-server -p storage-server -p unified-server -p lidp --all-targets --all-features --no-deps -- -D warnings
cargo hack test --feature-powerset --workspace --all-targets
just crap
```

`just crap` again built all four SDK ESM packages successfully, then exited 101 at llvm-cov metadata on that same manifest. No Rust coverage or CRAP score was produced. No workaround, sibling manifest change, or isolated dependency substitution was made. The deployment builds/scenarios, export parity, and sibling powerset commands in the original report above are historical attempts, not newly passing checks.

Delivery: commit the focused test/status/report corrections on `unified-owner-lifecycle-review-fixes`, then merge that branch with `GIT_EDITOR=true` into `unified-owner-lifecycle` at the existing `of` worktree. Report the resulting merge SHA separately (embedding it in its own merged commit would be self-referential). Preserve sibling work, existing branch/worktrees, and leave cleanup to the parent.
