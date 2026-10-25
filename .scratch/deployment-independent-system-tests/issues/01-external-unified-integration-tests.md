# 01: Move Unified public-interface tests outside its implementation

**What to build:** Unified composition and public host-lifecycle tests run as external consumers, while private implementation checks remain focused unit tests and owner-only checks live with their owners. Establish an assertion-based coverage inventory so subsequent shared-scenario migrations preserve existing guarantees.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] Classify existing tests and support fixtures by their assertions: private unit, owner-only, Unified public integration, shared Node system scenario, or deployment-specific system scenario. Record existing coverage and intended disposition without treating filenames as ownership evidence.
- [x] Move public-interface Unified composition and host-lifecycle coverage to external integration tests using existing public interfaces; demonstrate that representative moved tests pass.
- [x] Retain genuinely private configuration, locking, journal, and recovery checks beside their implementation. Relocate owner-only assertions where practical without introducing direct server-to-server dependencies.
- [x] Use generated owner clients for ordinary supported HTTP operations in migrated tests. Reserve raw HTTP for intentional wire/routing assertions and use existing native protocol clients where appropriate.
- [x] Remove replaced test modules, duplicate helpers, and private runtime hooks made obsolete by this slice. Do not widen production visibility solely to accommodate tests.
- [x] Keep scenario fixtures minimal; do not build a new generic framework or reimplement domain behavior.
- [x] Preserve security, recovery, and failure assertions. Record any remaining coupling or deferred move with a concrete reason and replacement plan.
- [x] Run focused moved tests and basic affected-target checks; record commands and results. Do not run CRAP/coverage or feature-powerset testing.

## Comments

Claimed by ticket 01 agent on `refactor/system-tests-01`. Baseline `191962716` equals the integration tip. Scope: existing Unified test migration, inventory, obsolete hooks; ticket 02 owns new shared scenarios/fixtures.

## Answer

Implemented and validated the external Unified consumer slice. The four migrated
external targets, existing composition tests, basic CLI tests and CLI all-targets
check pass after integration `af62ed7bc`. Ticket 01 is resolved. This is focused
host-test validation, not full system-test or platform qualification.

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

### Initial validation (historical)

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

The initial validation gaps above are superseded by the final focused execution
below. Historical timeouts remain timeouts, not passes.


### Final focused validation after integration `af62ed7bc`

The worktree was clean and fast-forwarded to integration `af62ed7bc` before this
continuation. Loaded `tdd` and `ponytail`; used the already approved public seams.
Read ticket03's report and reused its warm shared target. All Cargo jobs below ran
sequentially with a 600-second tool deadline, using:

```text
--manifest-path /home/nathan/code/rust/aicacia/of-system-tests-01/Cargo.toml
--target-dir /home/nathan/code/rust/aicacia/of/target
-j8
```

| Command suffix after `cargo <subcommand>` and the common options | Result |
| --- | --- |
| `test -p unified-server --test http_host --test owner_routes --test setup_host --test bootstrap_config -- --test-threads=1` | **PASS**: bootstrap_config 2, http_host 2 parent tests plus 1 successfully executed isolated child, owner_routes 1, setup_host 2. Rebuild 14.96s; HTTP execution 53.43s; remaining targets each under 0.1s. |
| `test -p unified-server --test composition -- --test-threads=1` | **PASS**, 2 tests, 1.67s (3.98s build). |
| `check -p unified-server --features cli --all-targets` | **PASS**, 19.24s; includes retained unit-test compilation and CLI binary. |
| `test -p unified-server --features cli --lib cli::tests -- --test-threads=1` | **PASS**, 5 CLI tests, 0.04s (97s build). |

Also passed `cargo fmt --manifest-path
/home/nathan/code/rust/aicacia/of-system-tests-01/Cargo.toml --package unified-server
--check` and `git diff --check`. The four IdP transport tests already passed during
the initial implementation; not rerun unnecessarily.

#### Red/green fixture correction

The first actual four-target run compiled successfully and passed bootstrap_config
and the OpenAPI test, but HTTP Create stayed `unfinished` and the child failed its
60-second readiness bound. This exposed an introduced fixture regression, not an
assertion to weaken: IdP's keyring entry builder supplies a `target` modifier; the
public mock store rejects nonempty modifiers. The original private `SecretStore`
had deliberately stripped them. Installing the bare mock during migration lost
that required test-provider behavior.

`tests/http_host.rs` now contains a small test-local `CredentialStoreApi` adapter
that delegates to the existing mock and omits platform modifiers when building an
entry. It contains no fault injection, persistence replacement, ready-state
injection, production code or public API. It remains isolated in the existing
bounded child process. All original security, recovery, exact wire/status,
Create/reopen, CLI verification/serving, and TLS non-generation assertions remain.
The exact same four-target command then passed, including the previously failing
child. The child is marked ignored only to prevent unisolated direct execution;
the passing parent explicitly launches and verifies it.

No remaining concrete ticket01 validation blocker. Qualification is limited to the
focused tests on this Linux host with an explicit mock credential provider, not
real OS key storage/trust installation or universal N0 network availability. No
CRAP, powerset, full-workspace build or full retained recovery suite was run.
Shared system fixtures and coverage entries owned by ticket04 were not edited;
main's unrelated staged setup-simplification work was not touched. Only the owned
HTTP fixture and this ticket are included in the follow-up commit.
