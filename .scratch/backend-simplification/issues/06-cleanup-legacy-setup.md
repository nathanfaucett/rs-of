# Ticket 06: Cleanup Legacy Setup Code

**Status: ready-for-agent**

**Blocked by:** 04

## What it delivers

Complete removal of all over-engineered setup/admission/provisioning code from the unified codepath. The `setup` crate remains ONLY for independent deployments.

## Scope

### Delete from `unified-server` (already started in Ticket 04, verify complete):
- `src/setup_host.rs` — entire file
- `src/provisioning.rs` — entire file
- `src/bootstrap/` — entire directory
- `src/independent_create_host.rs`
- `src/independent_setup_host.rs`
- `src/node_join*.rs` (all variants)
- `src/setup_admission.rs` (if exists)
- `src/setup_capability.rs` (if exists)
- Any `LocalTls` usage in unified path

### Delete from workspace:
- `of/data/unified/` — entire directory (scattered `idp.setup-host`, `device.setup-host`, `storage.setup-host`, `endpoint.tls`)
- `of/crates/api/` — entire crate (contracts moved to `of-model`)
- `of/crates/idp-model/` — entire crate (moved to `of-model`)
- `of/crates/storage-model/` — entire crate (moved to `of-model`)
- `of/crates/management-model/` — entire crate (moved to `of-model`)

### Verify `setup` crate preservation:
- `of/crates/setup/` — **UNCHANGED**, used by independent deployments
- Confirm `unified-server` no longer depends on `setup` crate (`cargo tree -p unified-server` shows no `setup`)

### Clean up Cargo workspace:
- Remove deleted crates from `of/Cargo.toml` members
- Remove unused dependencies from remaining crates
- Run `cargo hack check --feature-powerset --all-targets` to verify no feature combinations break

## Acceptance Criteria

- `cargo check --workspace` passes
- `cargo tree -p unified-server` shows no `setup` dependency
- `of/data/unified/` directory does not exist
- `api`, `idp-model`, `storage-model`, `management-model` crates deleted
- No references to `SetupHost`, `IndependentCreateHost`, `SetupAdmission`, `SetupCapability` in unified-server code
- `setup` crate still builds and works for independent deployments
- Net file deletion: significant reduction in lines of code
