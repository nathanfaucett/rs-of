# 08: Isolate mock-provider fixtures across concurrent tests

**Type:** task

**Status:** resolved

**Triage:** ready-for-agent

**Priority:** high

**Blocked by:** none

## Problem

The feature matrix reports 19 missing-private-signer failures. Source inspection shows that `new_with_store` replaces a process-global mock-provider backend while repositories retain only a service name. Other live fixtures can replace that backend. The mechanism predates setup simplification, but the exact store-switch sequence in the failing run was not traced; do not attribute every matrix failure to it.

See [validation diagnosis](../evidence/07-validation-followup.md), especially the failure-family breakdown and pre-spec comparison, and [qualification ticket 07](07-qualify-simplified-workflow.md).

## Acceptance criteria

- [x] Reproduce cross-fixture backend replacement with a deterministic concurrent regression using isolated mock credentials, not real providers/profiles.
- [x] Isolate fixture/provider ownership at its actual seam; do not serialize the entire suite, add sleeps, suppress failures, or change production authorization merely to hide interference.
- [x] Verify distinct stores retain their own signers throughout overlapping fixture lifetimes and reopen.
- [x] Run affected owner-boundary tests with normal concurrency and record actual results, distinguishing other failure families.

## Comments

Claimed on branch `implement/setup-followup-08` in worktree `.worktrees/setup-08`; verified HEAD and integration merge-base are `da5bc63419623ab48f93dea14dfce20f3e10ffc9`. Scope: provider-isolation production/tests, this ticket, and `evidence/08-provider-isolation.md` only.

Follow-up from the scoped merge-readiness decision. This issue is not evidence that ticket 07 or the feature matrix passes.

## Answer

Resolved: [provider-isolation evidence](../evidence/08-provider-isolation.md). Deterministic channel-ordered two-store regression failed on the original implementation (A loads B's signer), then passed with repositories retaining their actual backend. Explicit injection no longer changes the process default; default private/raw repositories capture the startup provider. Default-opening owner fixtures share one stable startup mock, while intentional isolated startup-provider installations are now explicit.

Final focused results: 16 IdP service unit tests pass; affected owner-boundary run at normal concurrency passes 35 tests, with 15 explicit independent-composition/SDK scope exclusions; isolated HTTP Create/reopen parent and child pass. Strict library Clippy, formatting and diff checks pass. Strict test-target Clippy is blocked by the unchanged `oauth2/config.rs` items-after-test-module lint. No full matrix/coverage or real provider/profile/trust use. Historical readiness/request, SDK and reset-fixture families remain separate; ticket 07 is not qualified by this result. Map and other tickets intentionally untouched per assigned ownership.
