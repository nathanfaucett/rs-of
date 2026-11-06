# 04: Existing test updates

**What to build:** Update all existing `*.test.ts` files to use new navigation (NavSidebar) and vault filters (FilterSidebar).

**Blocked by:** 01, 02

**Status:** ready-for-agent

- [ ] All tests: navigate via NavSidebar (Vault/Generator/Settings/Trash)
- [ ] Generator tests: click NavSidebar Generator item
- [ ] Trash tests: click NavSidebar Trash item
- [ ] Settings tests (lock, backup, import, export): click NavSidebar Settings item first
- [ ] Vault filter tests: use FilterSidebar items (Folders, Types, Favorites)
- [ ] All tests pass with new navigation structure
- [ ] Playwright integration tests: locked state nav, both sidebars in vault, mobile drawers, keyboard, offline toast
