# 06: Verify unified-server Composition Respects Ownership Boundaries

**What to build:** The `unified-server` composition host routes requests to the four services by path prefix without merging ownership boundaries. The four service APIs (IdP, Management, Device, Storage) remain distinct even when composed.

**Blocked by:** 01, 02, 03, 04, 05 (all prior tickets must complete first)

**Status:** ready-for-agent

**Acceptance criteria:**
- [ ] `unified-server` routes `/idp`, `/management`, `/device`, `/storage` to their respective services independently
- [ ] An IdP token does not grant access to device routes without proper Management/Device-layer checks
- [ ] Shared database (`of-db`) is used but each service's data namespace remains separate
- [ ] `just unified` or `cargo run -p unified-server -- local` works and all four routes respond independently

**Blocked by:** 01, 02, 03, 04, 05
