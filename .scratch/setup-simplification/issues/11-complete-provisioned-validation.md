# 11: Complete matrix and CRAP on a sufficiently provisioned runner

**Type:** task

**Status:** open

**Triage:** ready-for-agent

**Priority:** routine qualification

**Blocked by:** 08, 10

## Problem

The feature-matrix attempt reached owner-boundary tests (18 passed / 26 failed / 6 ignored child entry points), then stopped before remaining feature combinations. Coverage compilation was safely stopped when available memory fell to approximately 1 GiB with swap full; no CRAP score exists. The previously truncated Swagger ZIP has been narrowly repaired from a CRC-verified complete local copy.

Read [validation evidence and commands](../evidence/07-validation-followup.md) and [qualification ticket 07](07-qualify-simplified-workflow.md). Human triage is for arranging runner resources and stable inputs, not for changing assertions. Coordinate [09](09-diagnose-readiness-and-request-timeouts.md) if unexplained readiness/request failures recur.

## Acceptance criteria

- [ ] Provide adequate measured memory/disk headroom and record immutable source, sibling dependency, toolchain, and lockfile inputs. Do not overwrite user changes to manufacture a clean snapshot.
- [x] Build required workspace SDK artifacts before their test scenarios.
- [ ] Run `cargo hack test --feature-powerset --all-targets` to completion and clearly label root versus workspace scope; do not call a partial first combination a full matrix.
- [ ] Run `just crap` to completion and capture coverage/CRAP results. Preserve build/test semantics rather than changing profiles, suppressing features, or skipping assertions for a green result.
- [ ] Record full outcomes and remaining generated-SDK Clippy/standalone setup-test failures separately. Update ticket 07 and the spec only when their acceptance evidence supports it.

## Comments

- 2026-10-26: Claimed for the authorized execution attempt on `implement/setup-followup-11`, initially at integration `94c602883`. Tickets 08 and 10 are resolved. Own only this ticket and `../evidence/11-validation-attempt.md`; no map/spec/ticket07 or production changes. Triage marks execution ownership, **not** a provisioned runner. Exclusive Rust slot; jobs=1, stack=67108864, shared target, bounded commands and owned-process-group resource guards. No unrelated process termination, cache deletion, profile/assertion/semantic changes or retry-to-green.

- Execution disposition: [ticket11 evidence](../evidence/11-validation-attempt.md). Frozen JS install and four owner-client ESM builds passed; built exports **5/5 passed**. Rust preflight measured 6.82 GiB available RAM, 136 KiB free swap, 195.73 GiB disk. Exhausted swap failed the safety gate, so **Cargo matrix was not launched (zero combinations)** and `just crap` was not run. No compiler/test failure, matrix completion or CRAP score claimed. Ticket returns to **open**, not resolved; runner provisioning remains unmet. Eight pass-generated tracked artifacts are preserved unstaged in the worktree; only ticket/evidence are commit scope. Owned JS process groups are absent; no Rust process was created. No retry-to-green or ticket09 diagnosis/fix.

This does not authorize real profiles, OS trust installation, or production provider changes. Packaged desktop smoke testing with a disposable profile remains a release gate outside injected/backend qualification; full GUI/platform/WAN certification is not inferred from these commands.
