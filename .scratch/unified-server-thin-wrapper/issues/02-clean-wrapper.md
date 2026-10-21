# 02: Clean unified-server wrapper after route removal

**What to build:** After the route modules are moved, purge orphaned references from `unified-server/src/lib.rs`, confirm `router.rs` is pure merge, confirm `runtime.rs` uses only service routers, and preserve wrapper-level wiring (`endpoint`, `protocol`, `security_sync`).

**Blocked by:** 01

**Status:** ready-for-agent

Work status: open

## Comments

Blocked by 01. No production wrapper cleanup was performed. The extra node-join router merge and feature module exports remain. Endpoint, protocol and security synchronization files were not changed. See [map](../map.md).

- [ ] `unified-server/src/lib.rs` has no references to moved route modules
- [ ] `router.rs` contains only `compose_router` with `idp.merge(management).merge(storage)`
- [ ] `runtime.rs` `build_hosted` builds router only from `idp_runtime.router()`, `management_router`, `storage_runtime.router()`; no `.merge()` for `node_joins` or any other non-service router
- [ ] `endpoint.rs`, `protocol.rs`, `security_sync.rs` remain intact
