**Status:** resolved

- [x] Each service crate's SQL modules are reachable under `ofdb` paths
- [x] Each service crate's `ofdb` feature replaces `replica` in its own manifests
- [x] Service crates compile and their tests pass with both names available
- [x] No replication/sync terminology was renamed

## Answer

Renamed `src/replica/` → `src/ofdb/` in device-service, idp-service, management-service.
Flipped Cargo features so `ofdb` is the real feature and `replica = ["ofdb"]` is the alias
(including cross-feature refs: `management-service/ofdb`, `idp-service/ofdb`, `of-model/ofdb`,
`db/ofdb`). Switched all in-crate `#[cfg(feature = "replica")]` to `#[cfg(feature = "ofdb")]`
and module declarations to `pub mod ofdb` with `pub use ofdb as replica` alias.
Batch-replaced all internal import paths:

- `crate::replica` → `crate::ofdb`
- `of_model::replica` → `of_model::ofdb`
- `of_model::idp::replica` → `of_model::idp::ofdb`
- `idp_service::replica` → `idp_service::ofdb`
- `management_service::replica` → `management_service::ofdb`

Workspace lib check passes. Isolated feature checks show pre-existing errors
(unrelated to this change; same errors existed on main with `replica` feature).
