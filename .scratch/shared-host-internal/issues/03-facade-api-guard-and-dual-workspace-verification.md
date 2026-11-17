# 03: Facade API-surface guard and dual-workspace verification

**What to build:** A guard (CI check or test) that fails if a redb shared-host type or shared constructor reappears in the `ofdb-sql` / `ofdb-kv` public API, plus final verification that both workspaces are green after 01 and 02.

**Blocked by:** 01, 02

**Status:** ready-for-agent

- [ ] Guard runs in CI and fails on `SharedDatabase` / `open_shared` in either facade's public API
- [ ] Full test suites pass in both the `ofdb` and `of` workspaces
- [ ] Shared-host integration tests pass via the internal crates: namespace isolation, durability across restart, retirement fence, host file retained
