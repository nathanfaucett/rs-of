# 03: Qualify retained restart and offline-independent Join

**What to build:** Shared public-interface scenarios prove that both deployment layouts reopen retained Nodes, admit a joining Node through an authorized Trusted Peer, and reopen the completed joining Node with its source stopped.

**Blocked by:** 02 — Qualify Create and ordinary use across both deployment layouts.

**Status:** resolved

- [x] Extend the existing shared scenarios and concrete deployment fixtures, rather than adding a separate assertion suite per layout.
- [x] Stop and restart a created Node with the same retained configuration, protected provider, and state; verify public readiness and ordinary authorized use without fresh setup or reseeding.
- [x] Exercise documented source acceptance, pending Join, and exact authorized Device approval through generated clients. Preserve the distinction between endpoint possession, Device approval, signer trust, and resource authorization.
- [x] Run Join through each layout's documented setup sequence and admission capabilities, then verify correlated Installation identity, readiness, and usable owner interfaces.
- [x] Stop the source after completed Join, reopen the target from its retained state, and demonstrate readiness and an ordinary authorized operation without source access.
- [x] Ensure the independent joining Node has the independently retained owner state required by the documented local-owner Join operation. Attaching HTTP consumers to the source's shared SQL databases does not count as offline-independent Node qualification.
- [x] Keep private signers local and do not import source credentials, write fixture completion markers, or bypass approval using database injection.
- [x] Remove directly replaced restart/Join scenarios and obsolete fixture helpers once replacement coverage passes; retain legitimate layout-specific recovery assertions.
- [x] Demonstrate the shared restart and approved-Join/source-offline-reopen scenarios passing in both layouts with bounded diagnostics and reliable teardown. Record exact commands and limits; no CRAP/coverage or feature-powerset tests.

## Comments

Claimed on `refactor/system-tests-03` from `a2b9bad75` using the pre-approved
public generated-client/native-protocol seam and the `tdd` skill.

The initial Management connection blocker was reproduced in the actual CLI and
reported before production changes were authorized. The user then explicitly
approved the minimal explicit socket-selector correction; that exception is now
recorded in the spec and implemented with mutual exclusion, finite deadlines,
identity schema validation and stateless reset checks. There is no fallback,
file reopening, fixture proxy, source credential import or source SQL sharing.

## Answer

Completed in `b6f9cfcbf` atop the earlier retained-provider/Unified work. The
approved explicit Management socket selector and stateless reset validation have
focused positive/negative tests. Both layouts pass shared retained restart and
authorized Join/source-offline reopen; independent Nodes retain their own IdP,
Device and Storage owner state and target-local Management credentials.

After merging current integration (`a2b9bad75`, already up to date), final tests
passed: **16 Management, 1 provider, 7 lifecycle**. An earlier full seven-scenario
run and a separate two-Join repeat also passed. Strict Clippy, checks, formatting
and dependency boundaries passed. No CRAP/powerset or integration/main merge/push.

Production N0 discovery still needs working external networking; earlier DNS
failures and an unexplained early source-Create transient remain explicitly
reported, not claims of unconditional reliability. No old recovery/private body
is wholly replaced and no helper became obsolete; retained tests are mapped in
[coverage](../coverage.md). Exact commands, failures, limits and approved exception:
[ticket03-report](../ticket03-report.md).
