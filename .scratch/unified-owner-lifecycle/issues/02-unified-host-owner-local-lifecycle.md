# 02: Run Installation Setup on the owner-local lifecycle in the Unified Host

**What to build:** The Unified Host completes a Node's Installation Setup the same way the standalone deployment does. It hosts the IdP, Device, and Storage setup owners in its own process, each behind its own setup capability, and Create and Join run through those owners. The operator supplies the base APIs of the four contexts and the secret codes once; the client presents each owner's operation and that owner's capability to that owner and no other; Management participates from the supplied configuration. The Unified Host's own parallel setup implementation — its operation journals, host metadata store, hosted setup worker, bootstrap configuration, credential bootstrap, bootstrap verification, and endpoint credential file — is deleted in the same change, so a host is never half-provisioned by two lifecycles.

**Blocked by:** 01

**Status:** resolved

Claimed on branch `unified-owner-lifecycle-02`, worktree `/home/nathan/code/rust/aicacia/of-owner-local-lifecycle`, based on integration `d6ea7373f`. Approved test seams: composition, existing owner contracts, cross-layout deployment suite.

- [x] The Unified Host serves the IdP, Device, and Storage setup surfaces on its single composed endpoint, each behind its own capability.
- [x] A capability issued to one owner is rejected by the other two owners.
- [x] No request carries more than one owner's capability, and the administrator password reaches only the IdP owner.
- [x] The operator supplies the base APIs and secret codes once, and Management serves with the supplied base API and client credentials without a separate operator step.
- [x] Fresh Create produces a ready Installation whose operator login works, and each owner publishes its own readiness.
- [x] An exact retry is accepted and a conflicting operation is rejected, with the same rules the standalone owners apply.
- [x] Join runs on the same lifecycle, retains the Trusted Peer and authenticated issuer with the operation, and the joined Node is usable offline after restart.
- [x] The Unified Host publishes its installation endpoint through the owner-local peer facts, and the canonical issuer is the IdP base.
- [x] The only certificate authority involved is the one for the local dynamic setup server; the owners hosted in the Unified Host require no additional authority.
- [x] The Unified Host contains no operation journal, host metadata store, hosted setup worker, bootstrap configuration, credential bootstrap, bootstrap verification, or endpoint credential file of its own.
- [x] A retained Unified Host installation from before this change is refused with an actionable message rather than silently re-provisioned.
- [x] The Unified Host's test surface is one composition test covering the composed path map, prefix composition, single endpoint, absence of double prefixing, and the setup surface paths, plus at most one focused live-reconciliation file.
- [x] Every behavior asserted by a deleted Unified Host test file is asserted at an owning seam, or is intentionally dropped with a recorded reason.
- [x] No dead code, unused configuration field, or orphaned public export remains in the crate.

## Comments

- Final review restored fresh composed Management version/OpenAPI metadata through the existing state-free owner router, while keeping runtime health unavailable until serving. The single composition test now covers this boundary. Implementation remains resolved, but this new regression has not compiled or run at the final tip because of the unrelated sibling manifest blocker; historical checks below are not final-tip validation. See [final review](../final-review.md).

## Answer

Replaced Create and Join atomically with the existing owner workers, three owner-local capabilities and one shared local authority. Management is composed from the supplied configuration and Device runtime connection/control plane without a separate operator step. Deleted the parallel lifecycle and its repeated tests; migrated compile-critical native callers without compatibility APIs. Previous retained layouts fail closed, including orphaned endpoint state.

Validation is qualified, not blanket-green: focused composition/live Create+Join/source-offline Management authorization, API contracts, all-feature workspace check, focused Clippy and unchanged offs powersets passed. Root/workspace powerset, CRAP, broader owner and native test limits remain explicit. See [validation](../ticket02-validation.md) and [deleted coverage mapping](../ticket02-deleted-coverage.md). Tickets 03/04 retain deployment-harness/full consumer/docs qualification; this ticket does not claim those migrations complete.
