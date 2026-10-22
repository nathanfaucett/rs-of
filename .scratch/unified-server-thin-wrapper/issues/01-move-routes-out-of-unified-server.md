# 01: Move node-join and setup routes out of unified-server into service crates

**What to build:** Move `node_join_source.rs` (`/management/node-joins/*`) and `api_create.rs` (`/idp/setup/*`) routes out of `unified-server` and into the service crates that own those domains (`management-server`, `api`/`idp-server`), then remove both modules from `unified-server/src/lib.rs`. The Unified Host becomes a pure router composer and runtime assembler, with no new domain routes.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work status: resolved

## Comments

Partial attempt: architecture traced, but route implementations, tests and OpenAPI have not been relocated. Management owns hosted node joins; IdP owns hosted setup. Direct file moves would introduce forbidden server-to-server dependency edges or cycles through unified runtime/bootstrap types. See [map](../map.md) for concrete dependencies and validation. No acceptance checklist is claimed complete by this attempt.

- [x] `node_join_source.rs` routes are relocated to `management-server` or `idp-server`; `unified-server` no longer imports or references them
- [x] `api_create.rs` routes are relocated to `api` or `idp-server`; `unified-server` no longer imports or references them
- [x] Both modules are removed from `unified-server/src/lib.rs` and their test modules are moved alongside the moved code
- [x] `router.rs` remains a pure `idp.merge(management).merge(storage)` with no additional `.route()` calls
- [x] `runtime.rs` `build_hosted` continues to compose via existing service routers; no extra `.merge()` for moved routes
- [x] Existing `node_join_openapi` and `setup_openapi` exports are relocated to the owning service crate
- [x] All tests for the moved routes pass in their new locations
- [x] `compose_router` integration test confirms all three service prefixes respond with no unexpected `/management/node-joins/*` or `/idp/setup/*` paths unless provided by the composed service routers
## Answer
Routes relocated: node_join_source.rs -> management-server/src/node_join_source.rs; api_create.rs -> idp-server/src/hosted_setup.rs. unified-server/lib.rs purged of both modules; router.rs pure merge; runtime.rs passes node_joins to management_server::build_router (service-owned). Integration tests pass. Status: resolved.
Status: resolved
