# 02: Run Installation Setup on the owner-local lifecycle in the Unified Host

**What to build:** The Unified Host completes a Node's Installation Setup the same way the standalone deployment does. It hosts the IdP, Device, and Storage setup owners in its own process, each behind its own setup capability, and Create and Join run through those owners. The operator supplies the base APIs of the four contexts and the secret codes once; the client presents each owner's operation and that owner's capability to that owner and no other; Management participates from the supplied configuration. The Unified Host's own parallel setup implementation — its operation journals, host metadata store, hosted setup worker, bootstrap configuration, credential bootstrap, bootstrap verification, and endpoint credential file — is deleted in the same change, so a host is never half-provisioned by two lifecycles.

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] The Unified Host serves the IdP, Device, and Storage setup surfaces on its single composed endpoint, each behind its own capability.
- [ ] A capability issued to one owner is rejected by the other two owners.
- [ ] No request carries more than one owner's capability, and the administrator password reaches only the IdP owner.
- [ ] The operator supplies the base APIs and secret codes once, and Management serves with the supplied base API and client credentials without a separate operator step.
- [ ] Fresh Create produces a ready Installation whose operator login works, and each owner publishes its own readiness.
- [ ] An exact retry is accepted and a conflicting operation is rejected, with the same rules the standalone owners apply.
- [ ] Join runs on the same lifecycle, retains the Trusted Peer and authenticated issuer with the operation, and the joined Node is usable offline after restart.
- [ ] The Unified Host publishes its installation endpoint through the owner-local peer facts, and the canonical issuer is the IdP base.
- [ ] The only certificate authority involved is the one for the local dynamic setup server; the owners hosted in the Unified Host require no additional authority.
- [ ] The Unified Host contains no operation journal, host metadata store, hosted setup worker, bootstrap configuration, credential bootstrap, bootstrap verification, or endpoint credential file of its own.
- [ ] A retained Unified Host installation from before this change is refused with an actionable message rather than silently re-provisioned.
- [ ] The Unified Host's test surface is one composition test covering the composed path map, prefix composition, single endpoint, absence of double prefixing, and the setup surface paths, plus at most one focused live-reconciliation file.
- [ ] Every behavior asserted by a deleted Unified Host test file is asserted at an owning seam, or is intentionally dropped with a recorded reason.
- [ ] No dead code, unused configuration field, or orphaned public export remains in the crate.
