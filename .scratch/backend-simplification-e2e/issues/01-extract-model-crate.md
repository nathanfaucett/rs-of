# Ticket 01: Extract Single Model Crate

**Status: ready-for-agent**

**Blocked by:** (none)

## What it delivers

A single `of-model` crate containing all shared domain types, OpenAPI contracts, config shapes, and error types. All server crates and clients depend only on this crate for shared types. Eliminates duplicate definitions across `api`, `idp-model`, `storage-model`, `management-model`.

**Breaking changes expected** — rename types, move modules, change signatures. No compat shims.

## Scope

- Create new `of/crates/of-model` (or expand existing `model`)
- Move from `api`: contract definitions, `LocalTls`, `ServerConfig`, setup file utilities
- Move from `idp-model`: all identity/domain types, OAuth2/OIDC contracts
- Move from `storage-model`: all storage/domain types, sync protocols
- Move from `management-model`: management types, permission contracts
- Define `Database` trait (minimal: `txn()`, owner-specific methods)
- Publish as `of-model` with `no_std` + `serde` features
- Update all dependent crates (`idp-server`, `device-server`, `management-server`, `storage-server`, `unified-server`, clients) to import from `of-model`
- **Delete** duplicate type definitions from source crates (`api`, `idp-model`, `storage-model`, `management-model`)
- **Delete** `idp-model`, `storage-model`, `management-model` crates if empty after move

## Acceptance Criteria

- `cargo check --workspace` passes
- No type defined in more than one crate
- All OpenAPI contracts compile from single source
- `of-model` builds with `no_std` (alloc only)
- Clients (`idp-client`, `management-client`, `storage-client`) use `of-model` types
- Old model crates removed or empty
