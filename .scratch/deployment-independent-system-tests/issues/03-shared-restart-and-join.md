# 03: Qualify retained restart and offline-independent Join

**What to build:** Shared public-interface scenarios prove that both deployment layouts reopen retained Nodes, admit a joining Node through an authorized Trusted Peer, and reopen the completed joining Node with its source stopped.

**Blocked by:** 02 — Qualify Create and ordinary use across both deployment layouts.

**Status:** claimed

- [ ] Extend the existing shared scenarios and concrete deployment fixtures, rather than adding a separate assertion suite per layout.
- [ ] Stop and restart a created Node with the same retained configuration, protected provider, and state; verify public readiness and ordinary authorized use without fresh setup or reseeding.
- [ ] Exercise documented source acceptance, pending Join, and exact authorized Device approval through generated clients. Preserve the distinction between endpoint possession, Device approval, signer trust, and resource authorization.
- [ ] Run Join through each layout's documented setup sequence and admission capabilities, then verify correlated Installation identity, readiness, and usable owner interfaces.
- [ ] Stop the source after completed Join, reopen the target from its retained state, and demonstrate readiness and an ordinary authorized operation without source access.
- [ ] Ensure the independent joining Node has the independently retained owner state required by the documented local-owner Join operation. Attaching HTTP consumers to the source's shared SQL databases does not count as offline-independent Node qualification.
- [ ] Keep private signers local and do not import source credentials, write fixture completion markers, or bypass approval using database injection.
- [ ] Remove directly replaced restart/Join scenarios and obsolete fixture helpers once replacement coverage passes; retain legitimate layout-specific recovery assertions.
- [ ] Demonstrate the shared restart and approved-Join/source-offline-reopen scenarios passing in both layouts with bounded diagnostics and reliable teardown. Record exact commands and limits; no CRAP/coverage or feature-powerset tests.

## Comments

Claimed on `refactor/system-tests-03` from `a2b9bad75` using the pre-approved
public generated-client/native-protocol seam and the `tdd` skill.

The initial Management connection blocker was reproduced in the actual CLI and
reported before production changes were authorized. The user then explicitly
approved the minimal explicit socket-selector correction; that exception is now
recorded in the spec and implemented with mutual exclusion, finite deadlines,
identity schema validation and stateless reset checks. There is no fallback,
file reopening, fixture proxy, source credential import or source SQL sharing.

Current qualification includes the complete seven-test suite passing, then a
separate repeated pair of approved-Join/source-offline-reopen executions passing.
Final post-integration validation and report are being recorded before resolution.
No old recovery/private test body is wholly replaced, and no helper became obsolete;
see the updated coverage map for the assertion-level replacement boundaries.
