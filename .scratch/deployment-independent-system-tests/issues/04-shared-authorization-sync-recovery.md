# 04: Migrate authorization, synchronization, and recovery coverage

**What to build:** Applicable existing authorization, synchronization, learned-denial, Join rejection, and interrupted-retry scenarios verify Node behavior through public clients in both layouts, while genuinely private and layout-specific checks retain appropriate focused homes.

**Blocked by:** 03 — Qualify retained restart and offline-independent Join.

**Status:** claimed

- [ ] Reconcile the existing scenario inventory against the shared Create/restart/Join coverage and identify the remaining existing assertions in this ticket's scenario families. Do not add new product requirements or an exhaustive deployment cross-product.
- [ ] Migrate applicable permission and resource-denial assertions into shared scenarios using generated owner clients, preserving exact actor/audience/target checks rather than asserting private rows.
- [ ] Migrate applicable synchronization and learned-revocation coverage through existing native protocol clients. Preserve checks that learned denial affects subsequent operations, including existing connections where already covered.
- [ ] Preserve existing Join rejection and applicable interrupted/exact-retry behavior through public setup and Device contracts in both layouts. Do not equate cancellation acknowledgement with rollback or completed cleanup.
- [ ] Keep private storage-format/fault-injection tests with their implementation and truly layout-specific recovery checks in explicit deployment tests. Record concrete reasons for any case that cannot be shared rather than silently skipping it.
- [ ] Share behavioral assertions and vary only documented deployment/setup/lifecycle details. Do not create parallel bootstrap SQL implementations, security-policy helpers, or generic production interfaces to support the tests.
- [ ] Delete replaced scenarios, redundant fixtures, and obsolete private hooks as their replacements pass. Preserve meaningful negative coverage and scoped raw-HTTP wire tests.
- [ ] Run focused migrated scenarios in both applicable layouts, with safe diagnostics, bounded waits, and cleanup; record commands, results, and gaps. No CRAP/coverage or feature-powerset tests.

## Scope control

Keep this slice bounded to existing coverage. If the inventory demonstrates that these scenario families cannot fit one fresh implementation context, split it into scenario-family child tickets before implementation, each independently verifiable with explicit blockers. Do not replace this acceptance scope with an unbounded test-framework project.

## Claim

Claimed by ticket04 agent on `refactor/system-tests-04`, base `af62ed7bc`. Approved seams: generated owner HTTP clients and existing native protocol clients against actual processes. Targeted validation only; ticket01 external files and coverage edits are not owned here.

## Bounded execution

Split before implementation into sequential children 04a (permission/resource denial), 04b (learned denial), 04c (rejection/retry). Shared fixture edits are owned here; coverage.md is left to ticket01, with ticket04 inventory in its own report.
