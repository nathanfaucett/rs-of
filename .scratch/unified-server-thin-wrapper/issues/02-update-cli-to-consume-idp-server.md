# 02: Update unified-server to consume relocated logic from idp-server

**What to build:** Update the `unified-server` crate to consume the logic now hosted in `idp-server`. This involves updating module imports in the CLI and the library root.

**Target Files:**

- `unified-server/src/cli.rs`
- `unified-server/src/lib.rs`

**Blocked by:** 01: Relocate core logic modules to idp-serever

Status: resolved

- [x] Update `unified-server/src/cli.rs` to import modules from `idp_server` instead of `crate::`.
- [x] Update `unified-server/src/lib.rs` to remove `mod` declarations for the moved files and instead rely on them being part of the `idp_server` crate.
- [x] Verify the `unified-server` CLI still functions correctly.

## Answer

CLI imports the generic bootstrap workflows from `idp_server`; `UnifiedBootstrapHost` is the concrete composition adapter. `UnifiedConfig` retains file parsing and delegates bootstrap settings to IdP. Unified exports concrete aliases for its host API and retains the test-only isolated discovery preset. Removed declarations/files for relocated implementations; hosted setup/security/runtime adapters remain here.

Validation using the temporary `.validation/of/Cargo.toml` mirror: CLI-enabled IdP/unified all-target check and Clippy pass (existing warnings); Rust formatting and diff checks pass. Router composition 1/1, public composition integration 2/2, hosted setup 3/3 (isolated child also passes), credential-filtered tests 3/3, and setup-runtime adapter 1/1 pass. Local Create/Join/retry/offline-reopen coverage is recorded in ticket 01. The runtime acceptance fixture requires an unavailable built JavaScript SDK; baseline fails too. CLI `--help` smoke-run passes after completing compilation on a bounded retry.

No operational semantics were intentionally changed. Public construction of `UnifiedConfig` now uses `BootstrapSettings::into()` rather than a struct literal; field access remains available. See the map for baseline failures and outstanding dependency-boundary/broad-suite qualification. No ticket 03 cleanup was performed.
