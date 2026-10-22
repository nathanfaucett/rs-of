# 03: Purge unified-server of redundant modules and tests

**What to build:** Remove all remaining traces of the "Independent" logic and its associated tests from `unified-server` to achieve the "thin wrapper" state.

**Tasks:**

- Remove `independent_...` test modules.
- Remove the source files that were relocated.

**Blocked by:** 02: Update unified-server to consume relocated logic from idp-server

**Status:** ready-for-agent

- [ ] Remove all `mod independent_...;` declarations from `unified-server/src/lib.rs`.
- [ ] Delete all `independent_...` files from `unified-server/src/`.
- [ ] Delete the original logic files (e.g., `local_join.rs`, `local_provisioning.rs`, etc.) from `unified-server/src/` after verifying their new home in `idp-server`.
