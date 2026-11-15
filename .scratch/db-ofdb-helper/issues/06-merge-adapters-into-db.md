# 06: Merge adapters into `db` as concrete structs

**What to build:** Move the database adapters and local open helpers from the `embedded-db` crate into `db`: embedded file database, memory database, Postgres network database, remote ofdb SQL service database, private-storage open, and the local owner-database helper. Adapters are concrete structs exposing `client()` and, for file-backed ones, `storage()`; no trait is implemented for them. Server runtimes switch their injection point from `Option<Arc<dyn of_model::db::Database>>` to `Option<Arc<ofdb_sql::Database>>`, taking the storage handle from the concrete adapter when present.

**Blocked by:** 05 (Strip `of-model` to pure domain)

**Status:** ready-for-agent

- [ ] `db` exposes all four adapters plus the local helpers under their new names
- [ ] Each adapter opens its engine, runs `db::ofdb::up`, and returns errors instead of a half-initialized adapter
- [ ] All four server runtimes accept `Arc<ofdb_sql::Database>` injection and build with or without an injected database
- [ ] Storage runtime still receives the local `SharedDatabase` via the concrete adapter's `storage()`
- [ ] No `dyn` domain-database trait exists anywhere in the workspace
