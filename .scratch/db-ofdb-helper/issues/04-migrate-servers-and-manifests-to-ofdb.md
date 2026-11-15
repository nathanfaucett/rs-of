# 04: Migrate servers, workspace and app manifests to `ofdb`

**What to build:** Move all remaining call sites in the server binaries, the workspace root manifest, and the app manifests to the `ofdb` names: imports, module paths, and feature flags (`replica` -> `ofdb` on `of-model`, `db`, and the service crates). After this ticket no production code references the SQL-layer `replica` name directly (only ticket 01 aliases remain).

**Blocked by:** 02 (Migrate `of-model` to `ofdb`), 03 (Migrate the service crates to `ofdb`)

**Status:** ready-for-agent

- [ ] Server binaries and app code use `ofdb` module paths and features
- [ ] Workspace dev-dependency feature lists use `ofdb`
- [ ] Whole workspace compiles; tests pass
- [ ] Replication contract and replica-signer routes are unchanged
