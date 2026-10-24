# 01: Move Unified public-interface tests outside its implementation

**What to build:** Unified composition and public host-lifecycle tests run as external consumers, while private implementation checks remain focused unit tests and owner-only checks live with their owners. Establish an assertion-based coverage inventory so subsequent shared-scenario migrations preserve existing guarantees.

**Blocked by:** None (can start immediately).

**Status:** claimed

- [x] Classify existing tests and support fixtures by their assertions: private unit, owner-only, Unified public integration, shared Node system scenario, or deployment-specific system scenario. Record existing coverage and intended disposition without treating filenames as ownership evidence.
- [x] Move public-interface Unified composition and host-lifecycle coverage to external integration tests using existing public interfaces; demonstrate that representative moved tests pass.
- [x] Retain genuinely private configuration, locking, journal, and recovery checks beside their implementation. Relocate owner-only assertions where practical without introducing direct server-to-server dependencies.
- [x] Use generated owner clients for ordinary supported HTTP operations in migrated tests. Reserve raw HTTP for intentional wire/routing assertions and use existing native protocol clients where appropriate.
- [x] Remove replaced test modules, duplicate helpers, and private runtime hooks made obsolete by this slice. Do not widen production visibility solely to accommodate tests.
- [x] Keep scenario fixtures minimal; do not build a new generic framework or reimplement domain behavior.
- [x] Preserve security, recovery, and failure assertions. Record any remaining coupling or deferred move with a concrete reason and replacement plan.
- [ ] Run focused moved tests and basic affected-target checks; record commands and results. Do not run CRAP/coverage or feature-powerset testing.

## Comments

Claimed by ticket 01 agent on `refactor/system-tests-01`. Baseline `191962716` equals the integration tip. Scope: existing Unified test migration, inventory, obsolete hooks; ticket 02 owns new shared scenarios/fixtures.

## Answer

Implemented the external Unified consumer slice; validation of moved Unified targets
remains blocked by bounded build time. Keep this ticket claimed until those tests
run successfully; do not treat this as full system-test qualification.

- Added external `http_host`, `owner_routes`, `setup_host`, and `bootstrap_config`
  integration targets. Removed their original unit-test bodies, preserving the
  private origin validator, metadata corruption, revocation and fault-injection tests.
- Relocated all four generated transport tests and their fixture to the IdP client
  owner (`crates/idp-client/tests/transport.rs`), removing Unified's old module.
- Generated owner clients now consume ordinary HTTP readiness, completed Create
  retry and health. Exact wire/routing/status assertions remain raw intentionally.
- Public mock key store replaces the HTTP child's private fault-injection store and
  discovery hook. No production API widened. Existing runtime hooks still have
  private callers; see the explicit removal conditions in the inventory.
- [Assertion inventory and concrete remaining coupling/replacement plan](../coverage.md).
  Existing shared Node/owner/private scenarios remain enabled, not silently deleted.
- Complexity/risk: low structural complexity; moderate risk from subprocess test
  names, process-global provider state, and production endpoint behavior now being
  exercised without `cfg(test)`. Preserved child isolation and bounded waits.

### Validation

Commands ran from the main checkout with `--manifest-path
/home/nathan/code/rust/aicacia/of-system-tests-01/Cargo.toml`; all edits are in this
worktree. No neighboring repository modified. No CRAP/powerset or workspace build.

- PASS: `cargo fmt --manifest-path /home/nathan/code/rust/aicacia/of-system-tests-01/Cargo.toml --all -- --check`.
- PASS: `git diff --check` in this worktree.
- PASS (4 tests, 0 failures): `cargo test --manifest-path /home/nathan/code/rust/aicacia/of-system-tests-01/Cargo.toml -p of-idp-client --test transport --target-dir /home/nathan/code/rust/aicacia/of-system-tests-01/target/client --config 'profile.dev.package."*".opt-level=0' -j 2`.
- TIMEOUT 200s: Unified `--test http_host --test owner_routes --test setup_host
  --test composition --no-run`, shared `/home/nathan/code/rust/aicacia/of/target`.
  Reached Unified test code; fixed the one poll-state unused-assignment warning.
- TIMEOUT 90s: same focused no-run set plus `--test bootstrap_config` on shared target;
  recompiling dependencies. No compile error reported.
- TIMEOUT 90s: IdP transport test on shared target waited for Cargo build lock.
  Switched to isolated target for the passing transport run above.
- TIMEOUT 90s: `cargo check` with the same worktree manifest, `-p unified-server
  --tests --target-dir /home/nathan/code/rust/aicacia/of-system-tests-01/target/client
  --config 'profile.dev.package."*".opt-level=0' -j 2`; dependency compilation only.
- TIMEOUT 90s: isolated-target Unified focused test set (`composition`, `setup_host`,
  `owner_routes`, `bootstrap_config`, `http_host`), same profile override, `-j 8
  -- --test-threads=1`; dependency compilation only.
- TIMEOUT 90s: isolated-target `-p unified-server --test bootstrap_config`, same
  profile override, `-j 8`; reached Device/Storage service compilation, no test execution.

Remaining concrete validation: finish the five external Unified targets, check the
retained unit targets and basic CLI target. In particular the HTTP child now uses
production endpoint binding rather than private in-memory lookup; its Create/reopen
and CLI lifecycle must pass before closing the validation checkbox. Build artifacts
are retained for a follow-up; no validation timeout is counted as a pass.
