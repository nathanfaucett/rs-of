# 02: SQL protocol/server — associated Transaction type, RPITIT QueryTransaction

**What to build:** The query executor trait gains an associated `Transaction` type; `transaction()` returns `impl Future<Output = Result<Self::Transaction, ...>>` instead of a boxed trait object. The query transaction trait's methods (`execute`, `row_conflicts`, `row_conflict_values`, `commit`, `rollback`) return lifetime-capturing `impl Future + Send`. The gRPC query service holds the concrete associated transaction (it is already generic over the executor). The engine-backed transaction impl and the remote test doubles in the SQL facade's migration tests implement the new signatures, and the facade's conflict-inspection helpers stop casting through a boxed future alias.

**Blocked by:** 01 (engine stage helpers change signature first)

**Status:** ready-for-agent

- [ ] No `dyn QueryTransaction` / `Box<dyn QueryTransaction>` anywhere in the workspace
- [ ] ofdb-sql-protocol, ofdb-sql-server, and the ofdb-sql remote tests compile
- [ ] gRPC server tests and migration outcome tests pass; clippy/rustfmt clean
