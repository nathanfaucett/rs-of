# 01: Remove shared-host API from the published facades

**What to build:** In the ofdb crate tree, `ofdb-sql` and `ofdb-kv` expose only the storage-agnostic `Database` control handle and the `Client` query handle. The `SharedDatabase` re-export and the `Database::open_shared` constructor are removed from both facades; shared-host behavior tests move to the internal engine crates (`ofdb-btree-redb`, `ofdb-sql-engine-redb`, SQL history) so namespace isolation, restart durability, and the retirement fence stay covered there.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `ofdb-sql` and `ofdb-kv` no longer export `SharedDatabase` or `open_shared` under the `redb` feature
- [ ] Facade `Database` still supports `open` / `open_existing` / `in_memory` with `client()` queries, transactions, commit, and rollback unchanged
- [ ] Shared-host integration tests (isolation, durability across restart, retirement fence) pass against the internal crates
- [ ] ofdb workspace builds and its full test suite passes
