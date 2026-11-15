**Status:** resolved

- [x] `of-model` compiles with `--no-default-features` and has no `ofdb-sql`/`btree-redb` dependencies
- [x] `db::ofdb::up` applies `of_model::ofdb::MIGRATION` and is the single runner used by all entry points
- [x] Schema validators are reachable from `db::ofdb` and take a plain SQL client
- [x] No owner-view trait remains in `of-model`

## Answer

Stripped `of-model` to pure domain:

- Shrunk `ofdb` feature to `["std"]` (no engine deps); kept `replica = ["ofdb"]` alias temporarily for `embedded-db` compat
- Removed optional deps `btree-redb`, `ofdb-sql`, `include_dir`
- Deleted `db.rs` (owner-view traits `Database`, `IdpDatabase`, `DeviceDatabase`, `StorageDatabase`, `ManagementDatabase` and `database!` macro)
- Deleted `error.rs` (SQL-layer `DatabaseError` re-export)
- Replaced `ofdb.rs` with minimal `pub const MIGRATION: &str = include_str!(...)`
- Deleted `idp/ofdb.rs` entirely (migration runner `up`, validators `validate_identity`, `SecurityTable`, `SecurityRow`, `allows_*` functions)
- Removed `FromRow` impls from `device/selection_policy.rs` (SQL-layer code)
- Updated `lib.rs`: removed `db`, `error` modules; made `ofdb` module unconditional
- Updated `idp/mod.rs`: removed `ofdb` module declaration
- Updated `embedded-db/Cargo.toml` to use `ofdb` feature instead of `replica`

Verified: `cargo check -p of-model --no-default-features` ✓, `--features ofdb` ✓, `--features replica` ✓; workspace lib check ✓.
