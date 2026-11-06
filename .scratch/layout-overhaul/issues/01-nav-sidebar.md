# 01: Persistent navigation sidebar

**What to build:** Left sidebar (240px/64px) with Vault, Generator, Settings, Trash — always visible in all auth states. Mobile drawer (< 768px). Hash-driven (`#vault`, `#generator`, `#settings`, `#trash`). Collapse toggle persists to localStorage.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] New `Layout.svelte`: composes NavSidebar + Main (slot for optional FilterSidebar + content)
- [ ] New `NavSidebar.svelte`: fixed left, app title, 4 nav items, collapse button, mobile drawer
- [ ] New `NavSidebarItem.svelte`: icon + label, active state, tooltip when collapsed
- [ ] `+layout.svelte` wraps app in Layout, provides nav context (activeView getter/setter)
- [ ] `+page.svelte` removes inline tab bar (lines 428-433), keeps `activeView` state
- [ ] NavSidebar renders in locked/select-vault/create-vault/unlock-vault/error states
- [ ] Collapse toggle: 240px ↔ 64px, persists to `localStorage['pm:nav:collapsed']`
- [ ] Mobile: hamburger button (where?) opens drawer; selection closes drawer and navigates
- [ ] Accessibility: `nav[aria-label="Main navigation"]`, `role="menuitem"`, `aria-current="page"`
- [ ] Keyboard: Tab/ArrowUp/Down between items, Enter/Space activates
- [ ] Reduced motion: transitions disabled via media query
- [ ] Unit tests: NavSidebar renders 4 items, active highlight, collapse, drawer, persistence
- [ ] Integration test: NavSidebar visible and functional in locked state
