# 09: Move schema/adapter tests into `db`

**What to build:** Relocate the adapter test suite and the unified-schema test into the `db` crate's tests, updated to the `ofdb` names and the concrete adapter API. Server constructor tests (local router/runtime with injected shared database and owner-file fallback) are updated to the new injection type.

**Blocked by:** 05 (Strip `of-model` to pure domain), 06 (Merge adapters into `db` as concrete structs), 07 (Delete `embedded-db`; rename package to `of-db`)

**Status:** ready-for-agent

- [ ] File and memory adapters: open, migrate, persist, reopen; repeated init is idempotent; failed migration returns an error; unsafe existing files are rejected
- [ ] All owner tables are reachable through the single shared client; cross-owner transaction commits and rolls back
- [ ] Remote loopback SQL server test passes; Postgres test exists and is ignored without `OF_TEST_POSTGRES_URL`
- [ ] Server constructor tests pass for both injected and owner-file modes
- [ ] Tests assert external behavior only (tables, round-trips, transactions, persistence), not internal structure
