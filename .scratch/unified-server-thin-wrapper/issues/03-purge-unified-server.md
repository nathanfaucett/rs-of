# 03: Purge unified-server of redundant modules and tests

**What to build:** Remove all remaining traces of the "Independent" logic and its associated tests from `unified-server` to achieve the "thin wrapper" state.

**Tasks:**

- Remove `independent_...` test modules.
- Remove the source files that were relocated.

**Blocked by:** 02: Update unified-server to consume relocated logic from idp-server

**Status:** resolved

- [x] Remove all `mod independent_...;` declarations from `unified-server/src/lib.rs`.
- [x] Delete all `independent_...` files from `unified-server/src/`.
- [x] Delete the original logic files (e.g., `local_join.rs`, `local_provisioning.rs`, etc.) from `unified-server/src/` after verifying their new home in `idp-server`.

## Answer

Removed the obsolete root `independent_*` modules and declarations. Retained concrete multi-owner regression coverage under `owner_composition_tests`, including subprocess filter wiring and transfer/restart helpers; relocated bootstrap workflow implementations remain owned by IdP.

Architectural violation FIXED: IdP no longer depends on management-server or storage-server. Minimal owner bootstrap settings and host provisioning capabilities replace those dependencies; the unchanged dependency checker passes and the IdP reference scan is clean. `UnifiedConfig` retains concrete management/storage configs, and the adapter applies normalized bootstrap settings without discarding nondefault server options or public references. Durable journal shapes and authorization checks are preserved.

Saved focused validation passes: config 5/5, bootstrap 3/3, composition integration 2/2, minimal IdP settings 1/1, and compiled owner-composition listing (14 tests). The listing's source mirror matches the final Rust sources. Final rustfmt and diff hygiene checks pass. Broad suites, feature powerset, and CRAP were not repeated per handoff; baseline local/HTTPS failures, missing SDK acceptance, and historical IdP fixture failure remain recorded in the map. `.validation` stays untracked for parent cleanup.
