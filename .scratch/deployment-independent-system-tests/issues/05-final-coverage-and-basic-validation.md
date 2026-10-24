# 05: Finish coverage reconciliation and basic validation

**What to build:** The reorganized suite has an auditable disposition for every original scenario, no obsolete private test scaffolding, and recorded basic qualification of shared Node behavior in both deployment layouts.

**Blocked by:** 01 — Move Unified public-interface tests outside its implementation; 04 — Migrate authorization, synchronization, and recovery coverage.

**Status:** ready-for-agent

- [ ] Reconcile the final suite against the original assertion-based inventory. Mark each scenario retained, moved, shared, replaced, or deferred with a concrete reason; identify replacement coverage for every removal.
- [ ] Confirm public-interface Unified tests are external, owner-only checks are appropriately owned, and genuine private unit/fault-injection checks remain focused rather than being exposed through new production APIs.
- [ ] Confirm shared full-Node scenarios use public generated clients/native protocol clients and run the same common assertions in both layouts. Audit raw HTTP uses for intentional wire-level exceptions, not supported-operation workarounds.
- [ ] Remove remaining unused runtime test hooks, fixture modules, duplicate scenario helpers, stale test declarations, and dependencies rendered obsolete by the migrations. Do not remove unrelated code or ongoing user work.
- [ ] Preserve Unified shared-router/listener and owner-handle lifecycle coverage, independent-process lifecycle/connectivity coverage, and explicit deployment-specific setup/recovery assertions.
- [ ] Run basic affected-crate default and CLI checks, focused external integration tests, and representative shared Create/use, restart, Join, and source-offline reopen scenarios in both layouts. Run relevant migrated authorization/synchronization/recovery checks as applicable.
- [ ] Run formatting and dependency-boundary checks, plus relevant contract/client checks when touched. Preserve existing generated JavaScript qualification without expanding every scenario into a language matrix.
- [ ] Run Cargo-heavy validation sequentially where needed. Do not run CRAP/coverage, cargo-hack feature-powerset testing, unrelated cleanup campaigns, or automatic commit/merge/push.
- [ ] Record exact commands, passing/failing results, timeouts, provider/trust substitutions, and remaining qualification limits. Do not claim full-suite, real-OS-provider, platform, remote-deployment, or performance certification from focused evidence.
- [ ] Demonstrate that this effort reduced private coupling and duplicated assertions, rather than merely moving files or maintaining separate Unified and standalone behavioral suites.
