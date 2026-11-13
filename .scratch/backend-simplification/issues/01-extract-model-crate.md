# Ticket 01: Extract Single Model Crate (`of-model`)

**Status: ready-for-agent**

**Blocked by:** (none)

## What it delivers

A single `of-model` crate containing all shared domain types, OpenAPI contracts, config shapes, error types, and the `Database` trait. All server crates and clients depend only on this crate for shared types. Eliminates duplicate definitions across `api`, `idp-model`, `storage-model`, `management-model`.

**Breaking changes expected** — rename types, move modules, change signatures. No compat shims.

## Scope

- Create new `of/crates/of-model` (no_std + serde features)
- Move from `api`: contract definitions, `LocalTls`, `ServerConfig`, setup file utilities
- Move from `idp-model`: all identity/domain types, OAuth2/OIDC contracts, replication types
- Move from `storage-model`: all storage/domain types, sync protocols
- Move from `management-model`: management types, permission contracts
- Define `Database` trait in `of-model/src/db.rs` with owner-specific sub-traits
- Write unified migration `replica_migrations/0001_init.up.sql` concatenating all existing migrations in dependency order
- Update all dependent crates (`idp-server`, `device-server`, `management-server`, `storage-server`, `unified-server`, clients) to import from `of-model`
- **Delete** duplicate type definitions from source crates
- **Delete** `idp-model`, `storage-model`, `management-model` crates if empty after move
- **Delete** `api` crate (contracts moved to `of-model`)

## Acceptance Criteria

- `cargo check --workspace` passes
- No type defined in more than one crate
- All OpenAPI contracts compile from single source (`of-model`)
- `of-model` builds with `no_std` (alloc only)
- Clients (`idp-client`, `management-client`, `storage-client`) use `of-model` types
- Old model crates removed or empty
- `of_model::replica::up()` runs migration against empty redb without error
