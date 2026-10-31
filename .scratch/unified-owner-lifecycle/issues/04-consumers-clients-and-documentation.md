# 04: Migrate consumers, clients, and documentation

**What to build:** Every consumer of the previous Unified Host setup surface uses the owner setup surfaces instead. The desktop application installs and resets a Node through the owners' setup surfaces, supplies the base APIs and secret codes once, installs the single local setup authority it uses, and drives reset through the owner-local reset surfaces. The generated clients and published API descriptions carry no stale path, and the documentation describes the converged design: one lifecycle, one capability per owner, one surface shape in both topologies, and one certificate authority.

**Blocked by:** 02, 03

**Status:** ready-for-agent

- [ ] The desktop application installs and resets a Node without relying on the previous IdP-prefixed setup path.
- [ ] The desktop application installs one local setup authority rather than one per owner.
- [ ] The regenerated clients and published API descriptions contain no stale setup path, and the refreshed snapshots match.
- [ ] No document describes a setup lifecycle, capability count, surface shape, or trust footprint that the converged implementation no longer has.
- [ ] The shared glossary names the three setup owners and no longer describes the previous single-capability Unified setup.
- [ ] The operator documentation, setup worker and API contract documents, security document's trust description, control-plane deployment document, operations document, and configuration samples match the new surfaces.
- [ ] The command line pins the local setup authority by path and never mutates operating system trust during setup.
