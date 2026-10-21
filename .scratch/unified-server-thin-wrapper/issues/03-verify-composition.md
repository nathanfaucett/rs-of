# 03: Verify wrapper composition intact

**What to build:** Integration test confirming the Unified Host responds to all three service prefixes (`/idp/`, `/management/`, `/storage/`) and does not expose unexpected new routes unless they are now provided by the composed service routers.

**Blocked by:** 02

**Status:** ready-for-agent

Work status: open

## Comments

Still blocked by 02; not resolved. Added `crates/unified-server/tests/composition.rs`, which passed against the public `compose_router` API with supplied test routers. It checks all three prefixes, absent feature paths, supplied feature paths, and duplicate-prefix rejection. This is characterization coverage, not a red-green implementation cycle or evidence that production service builders own the features. The unchanged composition unit test passed. The unchanged runtime test failed because this worktree lacks the built Management JavaScript client (`packages/management-client/esm/index.js`). See [map](../map.md) for full validation and bounded-command limitations.

- [ ] `compose_router` responds to `/idp/health`, `/management/health`, `/storage/databases` (or equivalent service routes)
- [ ] No unexpected `/management/node-joins/*` or `/idp/setup/*` paths respond unless those routes are now included in the composed service routers
- [ ] Existing `router.rs` tests (`compose_service_routers_without_rewriting_prefixes`) pass unchanged
- [ ] Existing `runtime.rs` tests (`builds_three_prefixed_services_with_one_endpoint`) pass unchanged
