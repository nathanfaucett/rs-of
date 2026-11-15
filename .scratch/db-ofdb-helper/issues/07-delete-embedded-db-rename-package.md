**Status:** resolved

- [x] `embedded-db` is gone from the workspace and no manifest references it
- [x] Dependents reference the package as `of-db` with lib name `db`; all `db::...` imports unchanged
- [x] Workspace compiles and tests pass

## Answer

Deleted `crates/embedded-db` directory and removed it from workspace members.
Renamed `db` package from `idp-db` to `of-db` (lib name remains `db`).
Updated all dependent manifests (device-server, idp-server, management-server, storage-server, device-service, idp-service, management-service, unified-server, storage-service, workspace root) to reference `package = "of-db"`.
Updated `storage-service` dev-dependency and source files to use `db::adapters::EmbeddedDatabase`.
Updated `storage-server` dependency from `idp-db` to `db` (of-db).

Workspace lib check passes.
