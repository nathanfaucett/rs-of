# 02: Vault filter sidebar (Folders, Types, Favorites)

**What to build:** Second sidebar in Vault view only — Folders, Types, Favorites (1Password-style). 240px/64px collapse, mobile drawer. Independent state from nav sidebar. Only mounts when `activeView === 'vault'`.

**Blocked by:** 01 (needs Layout + activeView context)

**Status:** ready-for-agent

- [ ] New `FilterSidebar.svelte`: vertical nav, 3 items, collapse toggle, mobile drawer
- [ ] New `FilterSidebarItem.svelte`: icon + label, active, tooltip when collapsed
- [ ] `Layout.svelte` conditionally renders FilterSidebar in Main when `activeView === 'vault'`
- [ ] Collapse button in FilterSidebar header toggles width
- [ ] Mobile: filter icon button (in PageHeader or content) opens drawer; selection closes drawer
- [ ] Persistence: `filterSidebarCollapsed` → `localStorage['pm:vault:filter:collapsed']`
- [ ] Restored on Vault mount; default `false` (desktop) / `true` (mobile)
- [ ] Responsive: 3 breakpoints per spec table
- [ ] Accessibility: `nav[aria-label="Vault filters"]`, `role="menuitem"`, `aria-current`, focus trap in drawer, `Esc` closes
- [ ] Keyboard: ArrowUp/Down between items, Enter/Space activates
- [ ] Reduced motion: transitions disabled via media query
- [ ] Unit tests: FilterSidebar mounts only in Vault, collapse, drawer, persistence
- [ ] Integration tests: desktop expanded/collapsed, mobile drawer, keyboard, persistence
