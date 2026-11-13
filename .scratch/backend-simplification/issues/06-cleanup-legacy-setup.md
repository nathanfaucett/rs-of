# Ticket 06: Cleanup Legacy Setup Code

**Status: claimed**

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

## Comments

Cleanup audited on `backend-simplification-cleanup`, based on integration
`64e44f11e`. Ticket 04 and model extraction already removed the legacy unified
files, four crates, workspace entries, and tracked unified data. No remaining
tracked data justified deletion. Removed only Management's unused direct `setup`
dependency and its lockfile edge; the setup crate and independent owners remain
unchanged. Updated root documentation to distinguish local composition from
independent setup.

**Not resolved:** required transitive setup paths remain through owner servers and
server-common. Full graph exclusion requires an owner/common module or feature
split outside this task. The unchanged setup library builds, but its test targets
fail. Workspace checking fails on sibling snapshot serde errors; full feature,
test, and CRAP checks did not complete. Existing independent HTTPS tests also
reference the deleted API fixture path. See the exact commands, results, and
scope limits in [cleanup validation](../cleanup-validation.md).

Resumed verification preserved the interrupted edits and reproduced the remaining
setup dependency, setup test compilation failures, and workspace snapshot serde
failures. The setup library builds. All spec-listed obsolete paths are absent;
no additional data or code deletion was needed. Rust fixes remain with the buildfix
owner. Ticket 06 remains claimed, not resolved.
