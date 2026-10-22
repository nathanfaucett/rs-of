# 01: Relocate core logic modules to idp-server

**What to build:** Move the "Independent" logic modules from `unified-server/src/` to `idp-server/src/`, and export them via `idp-server/src/lib.rs` so that the `unified-server` CLI can consume them.

**Target Modules:**

- `local_join.rs`
- `local_provisioning.rs`
- `bootstrap_config.rs`
- `bootstrap_verification.rs`
- `credential_bootstrap.rs`
- `issuer_discovery.rs`
- `local_tls.rs`

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Move `local_join.rs`, `local_provisioning.rs`, `bootstrap_config.rs`, `bootstrap_verification.rs`, `credential_bootstrap.rs`, `issuer_discovery.rs`, and `local_tls.rs` to `idp-server/src/`.
- [ ] Export these modules in `idp-server/src/lib.rs` (e.g., `pub mod local_join;` etc.).
