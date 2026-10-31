# 04: Migrate consumers, clients, and documentation

**What to build:** Every consumer of the previous Unified Host setup surface uses the owner setup surfaces instead. The desktop application installs and resets a Node through the owners' setup surfaces, supplies the base APIs and secret codes once, installs the single local setup authority it uses, and drives reset through the owner-local reset surfaces. The generated clients and published API descriptions carry no stale path, and the documentation describes the converged design: one lifecycle, one capability per owner, one surface shape in both topologies, and one certificate authority.

**Blocked by:** 02, 03

**Status:** resolved

- [x] The desktop application installs and resets a Node without relying on the previous IdP-prefixed setup path.
- [x] The desktop application installs one local setup authority rather than one per owner.
- [x] The regenerated clients and published API descriptions contain no stale setup path, and the refreshed snapshots match.
- [x] No document describes a setup lifecycle, capability count, surface shape, or trust footprint that the converged implementation no longer has.
- [x] The shared glossary names the three setup owners and no longer describes the previous single-capability Unified setup.
- [x] The operator documentation, setup worker and API contract documents, security document's trust description, control-plane deployment document, operations document, and configuration samples match the new surfaces.
- [x] The command line pins the local setup authority by path and never mutates operating system trust during setup.

## Comments

- Claimed on branch `unified-owner-lifecycle-04`, worktree `of-consumers-04`, from `6e3dfa4a6823a7dc2f4bfaa70be0e30948171072`. Owner-local reset; no broker.

## Answer

Implemented on `unified-owner-lifecycle-04` from integration `6e3dfa4a6823a7dc2f4bfaa70be0e30948171072`.
Bundled and remote setup now use the existing three owner-local client/session surfaces;
scoped native retrieval supplies one capability per owner, with only IdP receiving the
password. Connection settings accepts all four public context bases. Owner/public evidence
is correlated before login; no aggregate setup/status fallback or deployment broker remains.
Reset uses the already-migrated stopped-owner boundaries and preserves the shared CA.
Deleted the remaining unused HostedSetup implementation/contracts/tests and regenerated
both affected IdP SDK families. Updated current operator/API/security/control-plane/native
and configuration/glossary documents; source-sharing uses the full Device base plus pin.

[Validation report](../ticket04-validation.md): frontend/Svelte/build and selected SDK tests
pass; all current Cargo/export/powerset/CRAP gates remain blocked by the unrelated sibling
offs duplicate manifest key. Rust export parity/runtime are not newly qualified. Unchanged
generator-failure tests also expose their existing `--locked` expectation mismatch; no
unrelated generator or sibling edits were made. Ticket03's nine completed process scenarios
remain historical evidence at the integration base, not a rerun by this ticket.
