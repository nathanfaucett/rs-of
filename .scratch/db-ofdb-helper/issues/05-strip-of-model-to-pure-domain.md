# 05: Strip `of-model` to pure domain

**What to build:** Remove the owner-view database traits (`Database`, `IdpDatabase`, `DeviceDatabase`, `StorageDatabase`, `ManagementDatabase`) and the migration runner/validator modules from `of-model`. The domain crate keeps only structs, error types, and `ofdb::MIGRATION`. The migration runner (`up`, formerly `up`/`up_device`) and the schema validators move into the `db` helper crate as `db::ofdb::up(&client)` and `db::ofdb` validators. `of-model` must build with no default features and no engine dependencies.

**Blocked by:** 02 (Migrate `of-model` to `ofdb`)

**Status:** ready-for-agent

- [ ] `of-model` compiles with `--no-default-features` and has no `ofdb-sql`/`btree-redb` dependencies
- [ ] `db::ofdb::up` applies `of_model::ofdb::MIGRATION` and is the single runner used by all entry points
- [ ] Schema validators are reachable from `db::ofdb` and take a plain SQL client
- [ ] No owner-view trait remains in `of-model`
