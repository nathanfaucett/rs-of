# 02: Update unified-server to consume relocated logic from idp-server

**What to build:** Update the `unified-server` crate to consume the logic now hosted in `idp-server`. This involves updating module imports in the CLI and the library root.

**Target Files:**

- `unified-server/src/cli.rs`
- `unified-server/src/lib.rs`

**Blocked by:** 01: Relocate core logic modules to idp-serever

**Status:** ready-for-agent

- [ ] Update `unified-server/src/cli.rs` to import modules from `idp_server` instead of `crate::`.
- [ ] Update `unified-server/src/lib.rs` to remove `mod` declarations for the moved files and instead rely on them being part of the `idp_server` crate.
- [ ] Verify the `unified-server` CLI still functions correctly.
