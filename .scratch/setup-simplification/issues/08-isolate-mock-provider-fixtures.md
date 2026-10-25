# 08: Isolate mock-provider fixtures across concurrent tests

**Type:** task

**Status:** open

**Triage:** ready-for-agent

**Priority:** high

**Blocked by:** none

## Problem

The feature matrix reports 19 missing-private-signer failures. Source inspection shows that `new_with_store` replaces a process-global mock-provider backend while repositories retain only a service name. Other live fixtures can replace that backend. The mechanism predates setup simplification, but the exact store-switch sequence in the failing run was not traced; do not attribute every matrix failure to it.

See [validation diagnosis](../evidence/07-validation-followup.md), especially the failure-family breakdown and pre-spec comparison, and [qualification ticket 07](07-qualify-simplified-workflow.md).

## Acceptance criteria

- [ ] Reproduce cross-fixture backend replacement with a deterministic concurrent regression using isolated mock credentials, not real providers/profiles.
- [ ] Isolate fixture/provider ownership at its actual seam; do not serialize the entire suite, add sleeps, suppress failures, or change production authorization merely to hide interference.
- [ ] Verify distinct stores retain their own signers throughout overlapping fixture lifetimes and reopen.
- [ ] Run affected owner-boundary tests with normal concurrency and record actual results, distinguishing other failure families.

## Comments

Follow-up from the scoped merge-readiness decision. This issue is not evidence that ticket 07 or the feature matrix passes.
