# 10: Repair owner-boundary fixture and SDK build prerequisites

**Type:** task

**Status:** claimed

**Triage:** ready-for-agent

**Priority:** routine

**Blocked by:** none

## Problem

The matrix exposes two concrete validation problems:

1. The exported-JavaScript SDK scenario cannot load `packages/idp-client/esm/index.js`; it fails before exercising SDK API behavior.
2. The isolated legacy Join/reset fixture writes a version-1 record with historical fields that the unchanged parser refuses before deletion, but the fixture expects partial deletion. The failure reproduces in an isolated child.

See [validation diagnosis](../evidence/07-validation-followup.md), including pre-spec source comparison, and [qualification ticket 07](07-qualify-simplified-workflow.md).

## Acceptance criteria

- [x] Make the existing SDK build prerequisite explicit in the test/CI entry point, then execute the actual exported SDK scenario. Do not commit generated build output as a workaround. (Passed against an existing executable; rebuilt Rust entry point remains pending.)
- [x] Reconcile the reset fixture with the supported current contract, retaining a distinct assertion that unsupported/malformed retained evidence is refused safely. (Source/test repair complete; Rust execution deferred.)
- [x] Preserve provider-failure evidence, exact-reference recovery, and local deletion versus remote revocation distinctions. Do not add historical migration/compatibility support to make the old fixture pass.
- [ ] Run both affected scenarios and their neighboring safety cases; record actual results and unrelated failures.

## Comments

Routine test maintenance, not a reason to weaken reset semantics or declare all matrix failures unrelated.

Claimed on `implement/setup-followup-10` in `.worktrees/setup-10`; confirmed clean base `da5bc63419623ab48f93dea14dfce20f3e10ffc9` (integration tip at claim). Own SDK prerequisite test/CI entry points, reset fixture tests, this ticket and `evidence/10-fixture-prerequisites.md`. Provider ownership implementation belongs to ticket 08. Rust build/test validation deferred while ticket 08 owns the shared build slot; no real profile/provider/trust changes.

## Implementation handoff

[Exact evidence and pending commands](../evidence/10-fixture-prerequisites.md). Added explicit owner-client ESM prerequisites to test/coverage recipes and Cargo-test CI, fail-fast prerequisite diagnostics, and the actual root integration target to OpenAPI qualification. Replaced historical cleanup expectations with independent unsupported/malformed refusal assertions followed by current-record reset; retained ordinary startup refusal, provider failure, partial deletion retry and exact-reference fencing.

Actual checks: exported SDK router scenario **1 passed** using a pre-existing binary plus owned-worktree JS, package exports **5 passed**, neighboring contract checks **30 passed**, newer SDK **45 passed**, Storage **21 passed**, OIDC **15 passed**, touched Rust formatting and diff checks passed. SDK/OIDC initially needed the existing native-fetch ESM build; ancillary generator-failure invocation timed out and is not qualified. No generated outputs delivered.

**Ready for coordinated Rust validation, not resolved.** Revised Create/Join reset parents, neighboring Rust safety cases, rebuilt prerequisite assertions, feature matrix, clippy and CRAP remain unexecuted while ticket 08 owns the shared build slot. No acceptance claim for those gates; no map/other-ticket/provider-implementation edits.
