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

## Fog

- Whether the desktop application's reset flow needs an owner-local reset surface per owner or can drive
  the deployment reset once.
- The offline-first filesystem deployment scenarios need a two-Node topology for any cross-node
  synchronization assertion; single-Node resource, content, residency, deletion, and restart coverage is
  unambiguous.

## Frontier

Ticket 04 is the lowest-numbered open, unblocked ticket. Ticket 03 qualification is resolved with broader-gate limits recorded in its validation report; ticket 04 owns the remaining consumer/docs migration.
