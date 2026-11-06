Status: ready-for-agent

## Problem Statement

The password manager has navigation (Vault, Generator, Settings, Trash) only in the unlocked vault view as a tab bar. No persistent navigation in locked/select-vault states. OfflineStatus renders as a persistent banner. No filter sidebar for vault contents.

## Solution

**Persistent left sidebar for main navigation** — always visible, all auth states. No header tabs. Each page owns its own secondary UI (filters, etc.).

| View      | Main Nav (Sidebar)                | Page-specific UI                          |
| --------- | --------------------------------- | ----------------------------------------- |
| Vault     | Vault, Generator, Settings, Trash | Filter sidebar: Folders, Types, Favorites |
| Generator | Vault, Generator, Settings, Trash | (none)                                    |
| Settings  | Vault, Generator, Settings, Trash | (none)                                    |
| Trash     | Vault, Generator, Settings, Trash | (none)                                    |

Three changes:

1. **Persistent navigation sidebar** — Left sidebar (240px / 64px collapsed), 4 items, always renders; mobile drawer (< 768px); hash-driven
2. **Vault filter sidebar** — Second sidebar in Vault view only (Folders, Types, Favorites); collapsible; mobile drawer; independent collapse state
3. **OfflineStatus → toast** — Remove banner; push to Notifications; suppress in dev

## User Stories

1. As a user, I want a persistent left sidebar with Vault/Generator/Settings/Trash, so I can navigate from any screen (locked, unlocked, creating vault).
2. As a vault user, I want a second filter sidebar (Folders/Types/Favorites) like 1Password, so I can filter items.
3. As a mobile user, I want both sidebars as drawers (hamburger for nav, filter icon for vault filters).
4. As a desktop user, I want both sidebars collapsible to icons.
5. As a user, I want offline status as a toast, not a banner.

## Implementation Decisions

### Architecture

- **Layout seam**: `+layout.svelte` → `Layout.svelte` composes `NavSidebar` + `Main` (with optional `FilterSidebar` slot)
- **Nav state**: `activeView` in `+page.svelte` (or context) drives content; `NavSidebar` reads/writes via context
- **Filter sidebar state**: Local to Vault view — no global module
- **No header component** — app title lives in NavSidebar header area

### Components to Create

1. **Layout.svelte** — Root: NavSidebar + Main (slot for FilterSidebar + content)
2. **NavSidebar.svelte** — Fixed left: app title, 4 nav items, collapse toggle, mobile drawer
3. **NavSidebarItem.svelte** — Icon + label, active, tooltip when collapsed
4. **FilterSidebar.svelte** — Vault-only: Folders, Types, Favorites, collapse, mobile drawer
5. **FilterSidebarItem.svelte** — Same pattern as NavSidebarItem

### Components to Modify

1. **+layout.svelte** — Wrap in Layout, provide nav context
2. **+page.svelte** — Remove tab bar; keep `activeView`; render views
3. **OfflineStatus.svelte** — Headless; toast via Notifications; no-op in dev

### Responsive Behavior

| Breakpoint | Nav Sidebar                    | Filter Sidebar                 | Mobile            |
| ---------- | ------------------------------ | ------------------------------ | ----------------- |
| < 768px    | Hamburger → drawer             | Filter icon → drawer           | Two drawers       |
| 768–1024px | Collapsed (64px), click expand | Collapsed (64px), click expand | Drawers available |
| > 1024px   | Expanded (240px), collapsible  | Expanded (240px), collapsible  | —                 |

### Accessibility

- NavSidebar: `<nav aria-label="Main navigation">`, `role="menuitem"`, `aria-current="page"`
- FilterSidebar: `<nav aria-label="Vault filters">`, same roles
- Mobile drawers: `<dialog aria-modal="true">`, focus trap, `Esc` closes
- Keyboard: Tab/Arrows in both sidebars, Enter activates

### Persistence

- `navSidebarCollapsed` → `localStorage['pm:nav:collapsed']`
- `filterSidebarCollapsed` → `localStorage['pm:vault:filter:collapsed']`

## Testing Decisions

### Unit Tests

- NavSidebar: 4 items, active, collapse, drawer, persistence
- NavSidebarItem: icon/label, active, tooltip, keyboard
- FilterSidebar: mounts only in Vault, collapse, drawer, persistence
- OfflineStatus: no DOM, toasts on change, suppressed in dev

### Integration Tests

- Locked state: NavSidebar visible, all 4 views accessible
- Vault: Both sidebars rendered, independent collapse, mobile drawers
- Generator/Settings/Trash: Only NavSidebar, no filter sidebar
- Keyboard: NavSidebar → FilterSidebar → content
- Offline toast

### Existing Test Updates

- All tests use NavSidebar for navigation
- Vault filter tests use FilterSidebar

## Out of Scope

- Header/search bar
- Multi-level nav
- Drag-resize
- Route-level code splitting

## Further Notes

- 5 new components (Layout, NavSidebar, NavSidebarItem, FilterSidebar, FilterSidebarItem)
- NavSidebar replaces tab bar + adds persistence
- FilterSidebar is 1Password-style secondary sidebar
- No header = more vertical space for content
