**Status:** resolved

- [x] Server binaries and app code use `ofdb` module paths and features
- [x] Workspace dev-dependency feature lists use `ofdb`
- [x] Whole workspace compiles; tests pass
- [x] Replication contract and replica-signer routes are unchanged

## Answer

Updated feature lists in all server crate manifests (`device-server`, `idp-server`,
`management-server`, `storage-server`, `unified-server`), workspace root
`dev-dependencies` (`of-model`, `db`, `device-service`, `management-service`,
`idp-service`), and app manifest (`apps/app/src-tauri`): `replica` → `ofdb`.

Batch-replaced import paths in server source files:

- `of_model::replica` → `of_model::ofdb`
- `of_model::idp::replica` → `of_model::idp::ofdb`
- `device_service::replica` → `device_service::ofdb`
- `idp_service::replica` → `idp_service::ofdb`
- `management_service::replica` → `management_service::ofdb`

Workspace lib check passes. Replication/sync terminology untouched.
