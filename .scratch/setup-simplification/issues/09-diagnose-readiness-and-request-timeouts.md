# 09: Diagnose unexplained readiness and HTTPS request timeouts

**Type:** task

**Status:** open

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

No real user profile, OS trust store, or production provider changes are authorized by this issue. Passing repetitions alone do not prove the origin of historical failures.
