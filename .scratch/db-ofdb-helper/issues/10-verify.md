# 10: Verify

**What to build:** Run the full validation for the refactor: feature-powerset tests for `of-model` and the `of-db` helper crate, then workspace-wide tests including the server crates. Confirm `of-model` builds with no default features.

**Blocked by:** 05 (Strip `of-model` to pure domain), 06 (Merge adapters into `db`), 07 (Delete `embedded-db`; rename package), 08 (Contract — delete old `replica` names), 09 (Move tests into `db`). Also blocked externally: the `ofdb` workspace working tree must compile again (pre-existing breakage in its SQL-schema/btree crates, not caused by this effort).

**Status:** ready-for-agent

- [ ] `cargo hack test --feature-powerset` passes for `of-model` and `of-db` (`db`), all targets
- [ ] `of-model` compiles with `--no-default-features`
- [ ] Workspace test suite passes, including all four server crates and the unified server
- [ ] Postgres integration test passes against a disposable database when run explicitly
