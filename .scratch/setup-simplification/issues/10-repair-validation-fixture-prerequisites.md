# 10: Repair owner-boundary fixture and SDK build prerequisites

**Type:** task

**Status:** resolved

**Triage:** ready-for-agent

**Priority:** routine

**Blocked by:** none

## Problem

The matrix exposes two concrete validation problems:

1. The exported-JavaScript SDK scenario cannot load `packages/idp-client/esm/index.js`; it fails before exercising SDK API behavior.
2. The isolated legacy Join/reset fixture writes a version-1 record with historical fields that the unchanged parser refuses before deletion, but the fixture expects partial deletion. The failure reproduces in an isolated child.

See [validation diagnosis](../evidence/07-validation-followup.md), including pre-spec source comparison, and [qualification ticket 07](07-qualify-simplified-workflow.md).

## Acceptance criteria

- [x] Make the existing SDK build prerequisite explicit in the test/CI entry point, then execute the actual exported SDK scenario. Do not commit generated build output as a workaround. (Current-source rebuilt Rust entry point and actual exported JavaScript scenario passed after integrating ticket 08.)
- [x] Reconcile the reset fixture with the supported current contract, retaining a distinct assertion that unsupported/malformed retained evidence is refused safely. (Both revised Create/Join parents and isolated children passed, including safe-refusal and current-record cleanup assertions.)
- [x] Preserve provider-failure evidence, exact-reference recovery, and local deletion versus remote revocation distinctions. Do not add historical migration/compatibility support to make the old fixture pass.
- [x] Run both affected scenarios and their neighboring safety cases; record actual results and unrelated failures.

## Comments

Routine test maintenance, not a reason to weaken reset semantics or declare all matrix failures unrelated.

Claimed on `implement/setup-followup-10` in `.worktrees/setup-10`; confirmed clean base `da5bc63419623ab48f93dea14dfce20f3e10ffc9` (integration tip at claim). Own SDK prerequisite test/CI entry points, reset fixture tests, this ticket and `evidence/10-fixture-prerequisites.md`. Provider ownership implementation belongs to ticket 08. Rust build/test validation deferred while ticket 08 owns the shared build slot; no real profile/provider/trust changes.

## Implementation handoff

[Exact evidence and pending commands](../evidence/10-fixture-prerequisites.md). Added explicit owner-client ESM prerequisites to test/coverage recipes and Cargo-test CI, fail-fast prerequisite diagnostics, and the actual root integration target to OpenAPI qualification. Replaced historical cleanup expectations with independent unsupported/malformed refusal assertions followed by current-record reset; retained ordinary startup refusal, provider failure, partial deletion retry and exact-reference fencing.

Actual checks: exported SDK router scenario **1 passed** using a pre-existing binary plus owned-worktree JS, package exports **5 passed**, neighboring contract checks **30 passed**, newer SDK **45 passed**, Storage **21 passed**, OIDC **15 passed**, touched Rust formatting and diff checks passed. SDK/OIDC initially needed the existing native-fetch ESM build; ancillary generator-failure invocation timed out and is not qualified. No generated outputs delivered.

Initial source-only handoff deferred Rust execution while ticket 08 owned the shared build slot. See the completed follow-up below; no map/other-ticket/provider-implementation edits.

## Answer

Resolved after the user released the exclusive Rust slot and authorized focused validation. Fast-forward merged integration **`8ee26fb1f8d789840dc246b94cfa22f6fdb7e6a5`**, rebuilt current sources with shared target, jobs 2 and stack 67108864, and ran bounded serial tests:

- Actual exported SDK scenario: **1 passed** through the rebuilt ordinary Rust/Node entry point, no shim.
- Revised unsupported/malformed-record Create/Join reset parents: **both passed**, with isolated children and unchanged safety assertions.
- Ordinary Create/Join offline reopen and remote consumer reset: **3 parents and their children passed**.
- Native reset/recovery safety: **6 passed**; peer revocation/issuer/per-owner acknowledgement: **1 passed**.
- Package export checks: **5 passed** again; workspace formatting and diff checks passed. No owned test/source repair was necessary.

The initial native filter `setup_reset_tests` selected zero tests; enumerated current names and reran the correct `setup_reset::tests` filter. This is recorded honestly, not counted as passing coverage. No other failures or timeouts occurred in the current-source follow-up. [Evidence](../evidence/10-fixture-prerequisites.md) records exact names, environments, bounds, build/test times, provenance and cleanup.

No matrix, coverage/CRAP or full OpenAPI generation was run; those broader effort-level gates and other tickets' failures remain unqualified here. No real profile/provider/trust changes, historical compatibility, generated output or pnpm artifacts delivered.
