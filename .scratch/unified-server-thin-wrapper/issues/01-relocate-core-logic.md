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

Status: resolved

- [x] Move `local_join.rs`, `local_provisioning.rs`, `bootstrap_config.rs`, `bootstrap_verification.rs`, `credential_bootstrap.rs`, `issuer_discovery.rs`, and `local_tls.rs` to `idp-server/src/`.
- [x] Export these modules in `idp-server/src/lib.rs` (e.g., `pub mod local_join;` etc.).

## Answer

Moved host settings/journals, credential bootstrap, local Create/Join state machines, reset references, and issuer discovery into IdP. `BootstrapHost` supplies concrete endpoint/runtime/protocol capabilities; journal formats, ordering, validation, and fail-closed recovery are unchanged. Existing public workflow tests remain the test seams.

Two target refinements avoid moving host-only code into the service: `LocalTls` already lives in `api`, so IdP re-exports it and owns the bootstrap settings implementation; `bootstrap_verification.rs` remains unified because it probes and shuts down the concrete composed HTTPS router. IdP calls that capability through `BootstrapRuntime`. No obsolete local implementation remains in unified.

Validation: bootstrap settings 3/3; local provisioning 8/10 (both failures reproduced on baseline `8667a29c3`); HTTPS workflows 7/8 with one intentionally ignored child (approval timeout reproduced on baseline). Hosted Create/credential/recovery tests pass. No behavior regression identified in the moved-code comparison. See the map for broader validation limits. Ticket 03 and `independent_*` files are untouched.
