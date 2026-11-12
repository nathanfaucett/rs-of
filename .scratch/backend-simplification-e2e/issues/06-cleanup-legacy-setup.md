# Ticket 06: Cleanup Legacy Setup Code (DELETE, don't archive)

**Status: ready-for-agent**

**Blocked by:** 05

## What it delivers

**Complete removal** of all over-engineered setup/provisioning code from the unified codepath. The `setup` crate remains for independent deployments but is **no longer a dependency** of unified-server. No archiving — delete dead code.

## Scope

- **Delete from `unified-server` crate:**
  - `SetupHost`, `IndependentCreateHost` types and all methods
  - `setup_host.rs` entirely
  - `cli.rs` Create/Join commands and `submit`/`observe_completed` logic
  - `reject_previous_layout`, `setup_files` usage
  - `setup` crate dependency from `Cargo.toml`
  - `config.rs` `FileConfig` deserialization (old YAML config format)
  - `runtime.rs` if it contains setup-related composition logic

- **Delete `of/data/unified/` directory entirely** (scattered setup-host dirs, TLS files)

- **Delete from `setup` crate (keep crate for independent deployments):**
  - `independent_create.rs`, `independent_exchange.rs`, `independent_history.rs`
  - `independent_operation.rs`, `independent_peer.rs`, `independent_reset.rs`
  - `independent_setup.rs`, `setup_admission.rs`, `setup_server.rs`
  - `setup_install.rs`, `setup_routes.rs`, `setup_scope.rs`
  - All `*_tests.rs` files for the above
  - `canonical_openapi.rs`, `setup_https_fixture.rs`
  - Keep only: `lib.rs` re-exports, `local_tls.rs`, `setup_files.rs` (used by independent)

- **Delete from owner servers (idp/device/storage/management):**
  - `independent_create.rs`, `independent_join.rs`, `independent_join_worker.rs`
  - `independent_reset.rs`, `independent_identity.rs` (idp)
  - `startup.rs`, `startup_https_tests.rs`
  - `provisioning.rs` (replace with Database-trait version)
  - `bootstrap/` directory (idp)
  - `node_join*` files (idp/device) — unified doesn't need node join
  - `management_owner.rs`, `management_permission_client.rs` (idp) — unified uses shared state
  - `storage_owner.rs` (idp), `authority.rs` (device) — unified uses shared state

- **Update `justfile` / scripts** to use simplified binary only
- **Update README/docs** with new zero-config startup (delete old setup instructions)

## Acceptance Criteria

- `unified-server` **no longer depends on `setup` crate** (verify `cargo tree`)
- `of/data/unified/` directory **removed from repo**
- `cargo build --bin unified-server` succeeds **without setup features**
- **Zero dead code** in unified-server crate (`cargo unused` or manual review)
- Independent deployment path (using `setup` crate) **still works** — verify separately
- Documentation reflects **only** new simple startup; old setup docs deleted
- `git status` shows **net deletion** of files (more removed than added)
