# 19: Start and reload the app offline

**What to build:** Reliable startup and reload of a previously loaded application without network access, including required database/WASM assets.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Inspect the existing application delivery path and reuse its supported offline mechanisms. Cache the production application assets and required database/WASM resources without caching decrypted vault content, keys, or sensitive responses.
- [ ] After a successful initial load/install, start and reload with network access disabled, then create/select/unlock a test vault, edit a synthetic Login, lock, and reopen using real local persistence.
- [ ] Do not depend on a running development server, a remote identity service, or a successful network request during the qualified offline flows. A first-ever visit without cached assets is not an offline guarantee.
- [ ] Provide actionable states for missing/failed offline assets, denied local storage, quota limits, and browser limitations. Explain that browser-local storage can be evicted and is not a backup.
- [ ] Ensure application cache updates do not delete vault storage or mix incompatible asset sets. Handle update failure without silently damaging the usable cached application.
- [ ] Use repeatable real-browser checks against the built application, not mocked persistence alone. Use only synthetic test vaults; do not create replacement live comparison tabs.
- [ ] Establish an asset/test convention that later feature tickets can extend. Final qualification covers the full typed-item release after those tickets land.

## Comments
