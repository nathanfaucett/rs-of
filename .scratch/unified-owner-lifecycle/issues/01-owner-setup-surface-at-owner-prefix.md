# 01: Serve the owner setup surface at the owner prefix

**What to build:** Every setup owner serves its setup surface at its own composition prefix in both deployments — at the root of its own origin when standalone, and at the IdP, Device, or Storage prefix of the composed endpoint when hosted. The public path names the surface instead of the deployment topology that uses it, so a standalone owner and a composed owner expose the same contract at the same relative location. Generated clients and published API descriptions follow the surface.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] A standalone owner serves setup at the root of its own origin, with the same behavior and the same status codes it serves today.
- [x] A composed owner serves setup at its composition prefix, with the same behavior and the same status codes it serves today.
- [x] The owner setup path no longer carries the deployment-topology infix.
- [x] The generated clients and published API descriptions expose the renamed surface, and the refreshed snapshots match.
- [ ] The owner-local contract tests and the existing deployment scenarios pass after the rename.
- [x] No compatibility route, alias, or fallback for the previous path remains.

## Comments

- Claimed on `unified-owner-lifecycle-01`, based on `unified-owner-lifecycle` at `2d1105a6e`. Approved seams: owner-local setup HTTP contract and existing cross-layout deployment scenarios.

## Answer

Implemented the complete owner-local surface at `/setup/owner/...`, relative to the exact owner base: root-origin standalone owners and prefix-composed owners use the same contract. Both topology-named path segments are gone, including worker handoffs. Generated Rust/TypeScript operations are now `owner_setup` / `OwnerSetupApi`; obsolete modules/models and the duplicate neutral OpenAPI description were deleted. All callbacks, listener probes, application callers and active API documentation follow the rename, without compatibility routes.

The agreed owner-contract test was red (503 instead of 200), then green. It covers IdP/Device/Storage in both prefix layouts, admission status semantics and absence of all old routes. Canonical exports and both SDK families regenerate byte-for-byte; focused contract/client/UI/composition checks pass.

The implementation is resolved with **broader qualification limits**, not an all-green workspace claim. The deployment checkbox remains unchecked: independent scenarios passed on the first run, but Unified Join encountered its documented N0 network preflight failure; broad owner-composition, UI, generator and workspace gates expose unrelated baseline failures or timeouts. See [the exact commands, results, baseline comparisons and limits](../ticket01-validation.md). Unified worker composition/replacement remains ticket 02, not a partial lifecycle replacement in this ticket.
