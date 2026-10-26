# 09: Diagnose unexplained readiness and HTTPS request timeouts

**Type:** task

**Status:** claimed

**Triage:** ready-for-agent

**Priority:** medium; raise if reproduced by users

**Blocked by:** none

## Problem

Earlier public HTTPS Join runs intermittently timed out observing a source offer or completing ordinary authorized requests. The matrix also reported four independent-owner readiness failures and one HTTPS permission-request timeout. Some isolated representatives pass; no common cause or blanket unrelated-baseline attribution is established.

A separate router/admission shutdown ownership defect and its queued-reply cycle are fixed. Final-code Join suites pass 9/9 scenarios across three serial repeats; that does not establish the cause of every earlier timeout.

Read [reliability evidence](../evidence/07-reliability-followup.md), [validation diagnosis](../evidence/07-validation-followup.md), and [qualification ticket 07](07-qualify-simplified-workflow.md). Reuse the existing public HTTPS and deployment-independent system scenarios rather than creating another suite.

## Acceptance criteria

- [ ] Reproduce and localize remaining failures with fixed recorded code/dependency inputs; distinguish idle serial execution, concurrent test execution, and tracing overhead.
- [ ] Capture categorical phase/task timing without passwords, setup codes, bearer tokens, or request payload disclosure.
- [ ] Establish each diagnosed root cause with a focused failing regression before changing implementation.
- [ ] Preserve exact intent/identity, authorized approval, pinned-peer trust, current readiness, and existing request bounds; do not mask failures with sleeps, retries, or deadline inflation.
- [ ] Record repeated finalization, interruption/exact retry, and source-offline reopen results plus remaining limits. Update ticket 07 only for criteria supported by evidence.

## Comments

Claimed by Agent09 on `implement/setup-followup-09` in `.worktrees/setup-09`, confirmed clean base `8ee26fb1f8d789840dc246b94cfa22f6fdb7e6a5` (integration). Initial pass is source investigation and secret-free categorical timing only; Agent10 owns the Rust slot, so no builds/tests until explicitly released. No provider, reset fixture, SDK, bounds, retry, sleep, or behavioral changes. Localization and focused red regression remain prerequisites to any behavioral fix.

[Initial observation-only handoff and predeclared diagnostic cells](../evidence/09-timeout-localization.md): dependency-visible categorical approval timing, existing client start/end, and shared-budget independent readiness status/error timing were delivered without builds/tests while Agent10 held the slot.

**Authorized focused execution, 2026-10-26:** merged integration `e6c72aae991f833670cb4820471ab48ac73b6bfb`, then built and ran fixed-input serial, concurrent and traced cells separately (one each, no retries to green). [Actual results and categorical timeline](../evidence/09-timeout-localization.md#authorized-focused-execution-2026-10-26): untraced HTTPS Join **12/12 passed**; traced interruption **failed**, client approval timeout at **5001ms**, owner handler success at **5439ms** after cumulative enrollment/verification/refresh/reply work, operation-lock wait **0ms**. Independent representatives: S0/S1/C0 **5/5** each; C1 **4/5**, with a distinct stopped-Device Database lock on immediate reopen after awaited shutdown. C1 also observed one recovered IdP readiness request timeout, not a terminal shared-readiness assertion failure. No deterministic causal regression or behavioral fix yet; all acceptance boxes remain unchecked and ticket07 is unchanged. Formatting and strict production-library Clippy pass; test-target Clippy retains documented diagnostics outside the touched helper after one lint-only conditional cleanup. Final owner no-run build passes; execution cells tested the pre-cleanup immutable integration source. No bounds/retries/sleeps/provider/reset-fixture/SDK/OS changes or matrix/coverage.

**P3 review follow-up:** replaced ineffective env_logger humantime-dependent timestamp formatting with explicit std Unix-ms formatting, keeping the fixed categorical-only filter and no new dependency. One actual-logger output check demonstrated red before the formatter change and green afterward. [Ownership investigation and bounded diagnostic](../evidence/09-timeout-localization.md#p3-clock-correction-and-ownership-follow-up): traced real Device-store → embedded-client/NativeResource → redb ownership, including transaction-held locks; one traced concurrent Create pair observed both post-shutdown Device reopens succeeding, then both parents failed their unchanged 180s watchdogs. This is **not** a passing replay or a reproduced Database-lock root cause. No lifecycle/sibling fix or manufactured retained-handle regression. Next blocker is a failing reopen capture identifying the surviving inner native handle/client or write transaction before a deterministic gated regression and fix. Formatting/focused formatter check pass; scoped test Clippy retains documented unrelated diagnostics. Ticket09 remains claimed/unresolved; ticket07 is unchanged.

No real user profile, OS trust store, or production provider changes are authorized by this issue. Passing repetitions alone do not prove the origin of historical failures.
