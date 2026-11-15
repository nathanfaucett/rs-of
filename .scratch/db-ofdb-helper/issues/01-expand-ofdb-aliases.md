# 01: Expand — `ofdb` aliases beside `replica`

**What to build:** Introduce the new `ofdb` names (module re-exports and feature aliases) alongside every existing SQL-layer `replica` name in `of-model`, the service crates, and the `db` helper crate, so that both names compile side by side. No caller changes yet. Real replication terminology (`contract::replication`, replica-signer routes, ofdb sync internals) is not touched.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] Every existing `replica` module path also resolves under the `ofdb` name
- [x] Every `replica` cargo feature also exists as an `ofdb` feature alias
- [x] The workspace compiles exactly as before (old names still work everywhere)
- [x] No occurrence of replication/sync `replica` terminology was renamed

## Answer

Added `ofdb = ["replica"]` feature aliases in `of-model`, `db`, `device-service`,
`idp-service`, and `management-service` manifests, plus `pub use replica as ofdb`
re-exports in each crate root (`of_model::ofdb`, `of_model::idp::ofdb`,
`device_service::ofdb`, `idp_service::ofdb`, `management_service::ofdb`).
`cargo check --workspace` compiles as before; `--all-targets` shows the same 13
pre-existing errors as `main` (unrelated ofdb sync API drift in test targets).
Replication/sync `replica` names untouched.
