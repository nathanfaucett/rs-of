# 05: Finish coverage reconciliation and basic validation

**What to build:** The reorganized suite has an auditable disposition for every original scenario, no obsolete private test scaffolding, and recorded basic qualification of shared Node behavior in both deployment layouts.

**Blocked by:** 01 — Move Unified public-interface tests outside its implementation; 04 — Migrate authorization, synchronization, and recovery coverage.

**Status:** resolved

- [x] Reconcile the final suite against the original assertion-based inventory. Mark each scenario retained, moved, shared, replaced, or deferred with a concrete reason; identify replacement coverage for every removal.
- [x] Confirm public-interface Unified tests are external, owner-only checks are appropriately owned, and genuine private unit/fault-injection checks remain focused rather than being exposed through new production APIs.
- [x] Confirm shared full-Node scenarios use public generated clients/native protocol clients and run the same common assertions in both layouts. Audit raw HTTP uses for intentional wire-level exceptions, not supported-operation workarounds.
- [x] Remove remaining unused runtime test hooks, fixture modules, duplicate scenario helpers, stale test declarations, and dependencies rendered obsolete by the migrations. Do not remove unrelated code or ongoing user work.
- [x] Preserve Unified shared-router/listener and owner-handle lifecycle coverage, independent-process lifecycle/connectivity coverage, and explicit deployment-specific setup/recovery assertions.
- [x] Run basic affected-crate default and CLI checks, focused external integration tests, and representative shared Create/use, restart, Join, and source-offline reopen scenarios in both layouts. Run relevant migrated authorization/synchronization/recovery checks as applicable.
- [x] Run formatting and dependency-boundary checks, plus relevant contract/client checks when touched. Preserve existing generated JavaScript qualification without expanding every scenario into a language matrix.
- [x] Run Cargo-heavy validation sequentially where needed. Do not run CRAP/coverage, cargo-hack feature-powerset testing, unrelated cleanup campaigns, or automatic commit/merge/push.
- [x] Record exact commands, passing/failing results, timeouts, provider/trust substitutions, and remaining qualification limits. Do not claim full-suite, real-OS-provider, platform, remote-deployment, or performance certification from focused evidence.
- [x] Demonstrate that this effort reduced private coupling and duplicated assertions, rather than merely moving files or maintaining separate Unified and standalone behavioral suites.

## Claim

Claimed on refactor/system-tests-05, baseline 120f7953a, using the spec-approved generated owner HTTP/native protocol seams and tdd. Scope: actual remaining-scenario/raw-HTTP/hook audit, deletion only with verified lack of callers or assertion-equivalent replacement, bounded serial basic validation. No production changes beyond the already approved Management socket correction; main staged setup-simplification is excluded.

## Answer

Resolved for the bounded spec scope on integration `120f7953a`. Reconciled **all
155 original test declarations** against actual baseline source, with an explicit
per-scenario disposition and current raw-HTTP/hook audit. Mixed owner-fixture
relocation and cross-layout live synchronization remain explicitly deferred for
concrete private-seam reasons, not marked migrated. No original scenario silently
removed; ticket04's 35-line replaced block is mapped precisely.

Removed three unused TLS fixture dependency edges and two duplicate dev-client
declarations; replaced the final ordinary raw external health poll with the
generated client. All existing runtime hooks still have private callers. Corrected
one pre-existing Management OpenAPI description mismatch through existing SDK
generation; SDK diffs are exclusively generated comments, repeat deterministic.
No new production behavior or visibility; approved socket scope unchanged.

Final evidence: **9/9 shared actual-process scenarios**, all five external Unified
targets plus IdP transport, strict default/CLI library Clippy, serial **16** Management,
**1** provider, **5** CLI, built JS/Rust actual-router qualification, **43** script
tests (35 repeated after metadata), deterministic four-owner exports, formatting
and dependency boundaries pass. Retained live post-Join cases **2 passed** with
explicit 64 MiB stack. Earlier concurrent socket deadline failure, default-stack
live import abort and corrected metadata red are preserved, not relabelled passes.

Exact commands, bounds, results and qualification gaps: [ticket05 report](../ticket05-report.md).
Every original disposition, retained live/private reasons and removal replacements:
[coverage map](../coverage.md). Integration merge check: already up to date.
No CRAP/coverage/powerset/full expensive library suite, main merge or push.
