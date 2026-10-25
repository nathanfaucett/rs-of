# 10: Repair owner-boundary fixture and SDK build prerequisites

**Type:** task

**Status:** open

**Triage:** ready-for-agent

**Priority:** routine

**Blocked by:** none

## Problem

The matrix exposes two concrete validation problems:

1. The exported-JavaScript SDK scenario cannot load `packages/idp-client/esm/index.js`; it fails before exercising SDK API behavior.
2. The isolated legacy Join/reset fixture writes a version-1 record with historical fields that the unchanged parser refuses before deletion, but the fixture expects partial deletion. The failure reproduces in an isolated child.

See [validation diagnosis](../evidence/07-validation-followup.md), including pre-spec source comparison, and [qualification ticket 07](07-qualify-simplified-workflow.md).

## Acceptance criteria

- [ ] Make the existing SDK build prerequisite explicit in the test/CI entry point, then execute the actual exported SDK scenario. Do not commit generated build output as a workaround.
- [ ] Reconcile the reset fixture with the supported current contract, retaining a distinct assertion that unsupported/malformed retained evidence is refused safely.
- [ ] Preserve provider-failure evidence, exact-reference recovery, and local deletion versus remote revocation distinctions. Do not add historical migration/compatibility support to make the old fixture pass.
- [ ] Run both affected scenarios and their neighboring safety cases; record actual results and unrelated failures.

## Comments

Routine test maintenance, not a reason to weaken reset semantics or declare all matrix failures unrelated.
