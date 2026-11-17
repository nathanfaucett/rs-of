# 02: Rewire `of` unified mode to the internal crates

**What to build:** In the `of` workspace, local unified mode (`unified.redb`) composes all owners and resources through the internal `ofdb-btree-redb` / engine crates directly. The filesystem runtime, IdP private-key repository, db adapter, and server test fixtures no longer use any facade shared-host API. The vendored facade copies under `of/db/*` are updated to match the ofdb crate tree.

**Blocked by:** None (the `of` workspace compiles against its own vendored crate copies, so it can start in parallel with 01)

**Status:** ready-for-agent

- [ ] No `of` crate imports `SharedDatabase` or calls `open_shared` from `ofdb-sql` / `ofdb-kv`
- [ ] Vendored `of/db/sql` and `of/db/kv` facade copies match the ofdb crate tree
- [ ] Local unified mode still runs: control-plane SQL plus per-resource KV/SQL namespaces in one host file
- [ ] `of` workspace builds and the shared filesystem / IdP / SQL integration tests pass
