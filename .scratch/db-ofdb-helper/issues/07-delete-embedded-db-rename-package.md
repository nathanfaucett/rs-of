# 07: Delete `embedded-db`; rename package to `of-db`

**What to build:** Remove the `embedded-db` crate from the workspace and rename the `db` helper package from `idp-db` to `of-db` while keeping lib name `db`, so every existing `db::...` import keeps working. Update workspace members and every dependent manifest (server crates, services' dev-dependencies, root manifest).

**Blocked by:** 06 (Merge adapters into `db` as concrete structs)

**Status:** ready-for-agent

- [ ] `embedded-db` is gone from the workspace and no manifest references it
- [ ] Dependents reference the package as `of-db` with lib name `db`; all `db::...` imports unchanged
- [ ] Workspace compiles and tests pass
