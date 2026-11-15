# 01: Expand — `ofdb` aliases beside `replica`

**What to build:** Introduce the new `ofdb` names (module re-exports and feature aliases) alongside every existing SQL-layer `replica` name in `of-model`, the service crates, and the `db` helper crate, so that both names compile side by side. No caller changes yet. Real replication terminology (`contract::replication`, replica-signer routes, ofdb sync internals) is not touched.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Every existing `replica` module path also resolves under the `ofdb` name
- [ ] Every `replica` cargo feature also exists as an `ofdb` feature alias
- [ ] The workspace compiles exactly as before (old names still work everywhere)
- [ ] No occurrence of replication/sync `replica` terminology was renamed
