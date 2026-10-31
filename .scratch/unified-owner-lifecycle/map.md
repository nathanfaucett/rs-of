# Unified Host owner lifecycle convergence

Spec: [spec.md](spec.md)

## Notes

One Installation Setup lifecycle for both deployment topologies. The Unified Host stops owning a parallel
setup implementation and instead hosts the IdP, Device, and Storage setup owners in one process, each
behind its own setup capability, with its own admission, retained exact operation, readiness, and reset
boundary.

Every owner keeps one setup surface at its composition prefix in both deployments: at the root of its own
origin when standalone, at the IdP, Device, or Storage prefix of the composed endpoint when hosted. There
is no deployment-level setup broker and no aggregated readiness endpoint — the operator supplies the base
APIs and secret codes once, and the client presents each owner's operation and that owner's capability to
that owner and no other.

Trust stays singular: the only certificate authority is the one for the local dynamic setup server that
the desktop application talks to. Owners hosted inside the Unified Host sit behind it.

The Unified Host crate ends up holding composition, configuration, command line, installation endpoint
identity, and the owner setup surfaces it hosts. The parallel lifecycle, its journals, its metadata store,
its endpoint credential file, and the test files that repeat owner or deployment behavior are deleted. The
replacement is atomic: an owner worker and the previous hosted path share owner state, so a host must
never be half-provisioned by both.

The cross-layout deployment suite is the single qualification seam for both topologies and gains the
offline-first filesystem coverage that today exists only in-process. The filesystem crate's own in-process
suite stays unchanged.

## Decisions-so-far

- One lifecycle: the owner-local setup contract. No third variant, no compatibility shim. (confirmed)
- Three setup capabilities, one per owner, in every topology. A capability authorizes one owner only. (confirmed)
- The owner setup surface is the only setup surface, at the owner prefix in both topologies. The
  deployment-topology infix is dropped. (confirmed)
- No deployment-level setup broker and no aggregated readiness document — the client presents one owner's
  capability per request. (confirmed)
- The operator supplies the base APIs and secret codes once; Management is not a setup owner and
  participates from that input with no extra operator step. (confirmed)
- The only certificate authority is the local dynamic setup server's. (confirmed)
- The replacement is atomic; no expand–contract overlap between the two lifecycles. (confirmed)
- The cross-layout deployment suite is the single qualification seam for both topologies; the filesystem
  crate's in-process suite stays unchanged. (confirmed)

- [Ticket 01](issues/01-owner-setup-surface-at-owner-prefix.md) resolved: the full owner-relative surface is `/setup/owner/...`, including public facts and worker handoffs. Reuse the existing topology-neutral namespace instead of colliding with the host `/setup/...` lifecycle that ticket 02 deletes. One `owner_setup` generated group replaces the split neutral/independent descriptions; no old-path aliases remain.
- Ticket 01 qualification: root/composed owner contracts and old-route absence pass; canonical contracts and both SDK families regenerate identically. Broader baseline/network/time-limit failures remain explicit in [the validation report](ticket01-validation.md); this is not full deployment acceptance. Ticket 02 owns Unified worker wiring and atomic lifecycle replacement.

- [Ticket 02](issues/02-unified-host-owner-local-lifecycle.md) resolved: Unified Host composes the existing Create/Join owner workers behind three capabilities and one CA; the parallel lifecycle, host metadata and credential bootstrap are deleted atomically. Focused live coverage includes source-offline Management authorization. [Validation and limits](ticket02-validation.md); [deleted coverage mapping](ticket02-deleted-coverage.md). Tickets 03/04 own cross-layout harness and full consumer/docs migration.

- [Ticket 03](issues/03-cross-layout-lifecycle-and-filesystem-qualification.md) resolved: one shared owner-local production-process suite passes all nine scenarios, including filesystem content/residency/tombstones and retained/source-offline reads. The live target Join listener race is reproduced and fixed with bounded status polling; Unified all-feature runtime and unchanged offs powerset pass. [Validation and external manifest blocker](ticket03-validation.md).

- [Ticket 04](issues/04-consumers-clients-and-documentation.md) resolved: bundled/remote consumers share owner-local Create/Join, four configurable public bases, three scoped capabilities and one hosted authority. Deleted the remaining aggregate IdP contract/SDK and unused hosted implementation, migrated source sharing/approval and current docs/configuration. [Validation and exact external limits](ticket04-validation.md).
- Reset fog resolved by owner-local stopped reset boundaries: a full desktop reset invokes the existing IdP, Device and Storage resets separately after shutdown, retaining per-owner completion across failures and preserving shared TLS. No deployment reset/setup broker or HTTP reset surface is introduced.

- [Final four-finding review](final-review.md): owner reset now retains completion evidence until desktop acknowledgment, fresh Management serves state-free metadata, CLI exact retries observe retired owners without authority, and desktop reset retains shared TLS/listener identity. Independent review also corrected the startup/reset lifecycle test's obsolete whole-root deletion assertion. These are implementation corrections, not newly passing runtime evidence.

## Fog

No unresolved implementation choice remains for this effort. Final Rust compilation/runtime qualification is unavailable: the review-fix regressions have not compiled or run because the unrelated sibling offs manifest has a duplicate target-dependencies table. Historical ticket01–03 results do not validate the final tip. Cross-node filesystem synchronization, real GUI/provider/OS trust and platform qualification remain outside the bounded evidence; unchanged generator cold-cache limitations remain as recorded in ticket04. See [final-review.md](final-review.md) for exact rerun attempts and limits.

## Frontier

All four implementation tickets are resolved; this is not complete validated acceptance. The final review-fix delivery branch is `unified-owner-lifecycle-review-fixes`, with integration destination `unified-owner-lifecycle` in the existing `of` worktree. Full Cargo/export/powerset/CRAP qualification must be rerun after the unrelated sibling manifest edit is complete. Leave worktree/branch cleanup to the parent.
