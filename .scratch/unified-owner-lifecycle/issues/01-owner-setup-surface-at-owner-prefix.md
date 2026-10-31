# 01: Serve the owner setup surface at the owner prefix

**What to build:** Every setup owner serves its setup surface at its own composition prefix in both deployments — at the root of its own origin when standalone, and at the IdP, Device, or Storage prefix of the composed endpoint when hosted. The public path names the surface instead of the deployment topology that uses it, so a standalone owner and a composed owner expose the same contract at the same relative location. Generated clients and published API descriptions follow the surface.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A standalone owner serves setup at the root of its own origin, with the same behavior and the same status codes it serves today.
- [ ] A composed owner serves setup at its composition prefix, with the same behavior and the same status codes it serves today.
- [ ] The owner setup path no longer carries the deployment-topology infix.
- [ ] The generated clients and published API descriptions expose the renamed surface, and the refreshed snapshots match.
- [ ] The owner-local contract tests and the existing deployment scenarios pass after the rename.
- [ ] No compatibility route, alias, or fallback for the previous path remains.
