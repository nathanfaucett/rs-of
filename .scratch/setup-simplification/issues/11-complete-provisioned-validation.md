# 11: Complete matrix and CRAP on a sufficiently provisioned runner

**Type:** task

**Status:** open

**Triage:** ready-for-human

**Priority:** routine qualification

**Blocked by:** 08, 10

## Problem

The feature-matrix attempt reached owner-boundary tests (18 passed / 26 failed / 6 ignored child entry points), then stopped before remaining feature combinations. Coverage compilation was safely stopped when available memory fell to approximately 1 GiB with swap full; no CRAP score exists. The previously truncated Swagger ZIP has been narrowly repaired from a CRC-verified complete local copy.

Read [validation evidence and commands](../evidence/07-validation-followup.md) and [qualification ticket 07](07-qualify-simplified-workflow.md). Human triage is for arranging runner resources and stable inputs, not for changing assertions. Coordinate [09](09-diagnose-readiness-and-request-timeouts.md) if unexplained readiness/request failures recur.

## Acceptance criteria

- [ ] Provide adequate measured memory/disk headroom and record immutable source, sibling dependency, toolchain, and lockfile inputs. Do not overwrite user changes to manufacture a clean snapshot.
- [ ] Build required workspace SDK artifacts before their test scenarios.
- [ ] Run `cargo hack test --feature-powerset --all-targets` to completion and clearly label root versus workspace scope; do not call a partial first combination a full matrix.
- [ ] Run `just crap` to completion and capture coverage/CRAP results. Preserve build/test semantics rather than changing profiles, suppressing features, or skipping assertions for a green result.
- [ ] Record full outcomes and remaining generated-SDK Clippy/standalone setup-test failures separately. Update ticket 07 and the spec only when their acceptance evidence supports it.

## Comments

This does not authorize real profiles, OS trust installation, or production provider changes. Packaged desktop smoke testing with a disposable profile remains a release gate outside injected/backend qualification; full GUI/platform/WAN certification is not inferred from these commands.
