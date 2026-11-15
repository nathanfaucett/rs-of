**Status:** resolved

- [x] `of_model::ofdb::MIGRATION` resolves to the single init migration SQL
- [x] The `ofdb` feature on `of-model` enables the same things the `replica` feature did
- [x] All in-crate call sites use the `ofdb` name
- [x] Crate still compiles with the alias in place

## Answer

Renamed `replica.rs` → `ofdb.rs` and `idp/replica.rs` → `idp/ofdb.rs`. Removed
obsolete `replica_migrations/` and `device_migrations/`. Flipped Cargo features so
`ofdb` is the real feature and `replica = ["ofdb"]` is the alias. Updated all
in-crate `#[cfg(feature = "replica")]` to `#[cfg(feature = "ofdb")]` (modules
`db`, `error`, `ofdb`, and `device/selection_policy`). Added alias re-exports
`pub use ofdb as replica` gated on `replica` feature. Verified:

- `cargo check -p of-model --features ofdb` ✓
- `cargo check -p of-model --features replica` ✓ (alias)
- `cargo check -p of-model --no-default-features` ✓
- Workspace lib check ✓
