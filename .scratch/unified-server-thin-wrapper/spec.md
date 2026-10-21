Status: ready-for-agent

## Problem Statement

The `unified-server` crate is intended to be a thin wrapper that composes the IdP, Management, and Storage service routers into a single Unified Host endpoint. Instead, it hosts new HTTP route modules (`node_join_source.rs`, `api_create.rs`) that expose `/management/node-joins/*` and `/idp/setup/*` endpoints not present in the individual service crates. This violates the wrapper contract: the Unified Host should coordinate existing service contexts without introducing new domain routes.

## Solution

Move all new HTTP route modules out of `unified-server` and into the appropriate service crates (`idp-server`, `management-server`, or `api`), then reduce `unified-server` to pure router composition (`compose_router`) and runtime assembly (`build_hosted`). The Unified Host retains only prefix wiring (`/idp/`, `/management/`, `/storage/`) and endpoint/protocol adapters required to bind the three services together.

## User Stories

1. As a Unified Host operator, I want `unified-server` to expose only the composed routes of IdP, Management, and Storage, so that service ownership boundaries remain clear.
2. As an IdP service maintainer, I want node-join approval routes to live in `idp-server`, so that identity and enrollment logic stay co-located.
3. As a Management service maintainer, I want node-join pending/approve/reject routes to live in `management-server`, so that device approval stays co-located.
4. As an Installation Setup maintainer, I want setup status/create/join/admission routes to live in the setup service (`api` or `idp-server`), so that bootstrap logic stays co-located.
5. As a system architect, I want `unified-server` to contain no new feature modules, so that the wrapper can be replaced or audited independently.
6. As a developer, I want `router.rs` to remain a pure merge of three service routers, so that prefix composition is explicit and testable.
7. As a tester, I want existing router-composition tests (`compose_router`) to continue passing unchanged, so that wrapper behavior is preserved.
8. As an operator, I want the hosted `/management/node-joins/acceptance` endpoint to continue working after the move, so that production nodes are not disrupted.
9. As an operator, I want the hosted `/idp/setup/status` endpoint to continue working after the move, so that Installation Setup is not disrupted.
10. As a maintainer, I want `node_join_source.rs` to be removed from `unified-server/src/lib.rs`, so that no orphaned module references remain.
11. As a maintainer, I want `api_create.rs` to be removed from `unified-server/src/lib.rs`, so that no orphaned module references remain.
12. As a maintainer, I want `unified-server` to retain only `router`, `runtime`, `endpoint`, `protocol`, and `security_sync` wiring, so that the wrapper's scope is minimal.

## Implementation Decisions

- The Unified Host is defined in `GLOSSARY.md` as composing IdP, Management, and Storage contexts without merging state ownership; new routes violate this definition.
- `node_join_source.rs` defines `/management/node-joins/acceptance`, `/pending`, `/{id}/approve`, `/{id}/reject`; these belong in `management-server` (device approval) or `idp-server` (identity enrollment) depending on which domain owns the operation.
- `api_create.rs` defines `/idp/setup/status`, `/create`, `/join`, `/cancel`, `/admission`, `/intent`; these belong in `api` or `idp-server` as Installation Setup routes.
- `router.rs` must remain a pure `idp.merge(management).merge(storage)` with no additional `.route()` calls.
- `runtime.rs` `build_hosted` must continue to call `management_server::build_router`, `build_storage_runtime`, and `build_idp_runtime`, then compose with `compose_router`; any moved routes must be included via those service routers, not via extra `.merge()` calls in `runtime.rs`.
- Existing `node_join_openapi` and `setup_openapi` exports from `unified-server/src/lib.rs` must be preserved or relocated to the service crate that owns the routes.
- The `ready-for-agent` triage label applies; no additional triage is needed.

## Testing Decisions

- A good test verifies only external behavior: the composed router responds to `/idp/health`, `/management/health`, `/storage/databases`, and any moved routes at their new prefixes, without inspecting internal module paths.
- Prior art: `router.rs` tests (`compose_service_routers_without_rewriting_prefixes`) and `runtime.rs` tests (`builds_three_prefixed_services_with_one_endpoint`) already verify wrapper composition.
- After the move, add one integration test in `unified-server` confirming `compose_router` produces a router that responds to all three service prefixes and no unexpected `/management/node-joins/*` or `/idp/setup/*` paths unless those routes are now provided by the composed service routers.
- Do not test implementation details of `node_join_source.rs` or `api_create.rs` inside `unified-server`; those tests belong with the moved modules.

## Out of Scope

- Changing the actual business logic of node-join approval or Installation Setup.
- Changing service state ownership (IdP owns identity, Management owns Devices, Storage owns resources).
- Changing protocol-level endpoint or replication behavior (`protocol.rs`, `security_sync.rs`).
- Changing the `ofnet`/`iroh-chain` endpoint mesh mechanics.
- Rewriting the OpenAPI generation pipeline; only the module location changes.

## Further Notes

- The `unified-server` `endpoint.rs` module manages the `UnifiedEndpoint` identity and key publication; this is wrapper-level infrastructure and should remain.
- The `unified-server` `security_sync.rs` and `security_candidates.rs` modules manage cross-service security synchronization; these are wrapper-level coordination and should remain.
- The distinction between "independent" routes (`/independent/node-joins/enroll`, `/independent/setup/*`) and "hosted" routes (`/management/node-joins/*`, `/idp/setup/*`) is already present in `docs/api.md`; the hosted versions must live in the service crates that provide the hosted APIs.
