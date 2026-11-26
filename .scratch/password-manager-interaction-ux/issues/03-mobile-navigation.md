# 03: Make mobile navigation accessible and non-obscuring

**What to build:** The small-screen navigation drawer manages focus and background content correctly, closes through keyboard and pointer paths, and returns focus to its opener. The opener no longer covers records, filters, or actions.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

## Scope

- Treat the open small-screen drawer as a modal navigation surface with inert background content.
- Move focus into the drawer, preserve arrow/Home/End navigation, and provide an explicit internal Close control.
- Support Escape from drawer controls, backdrop dismissal, route selection, desktop-resize close, and opener focus return.
- Reposition the opener or reserve layout space so it does not obscure page content.
- Preserve desktop collapse preferences and existing navigation destinations.

## Non-goals

- No navigation redesign, new routes, synchronization changes, or changes to desktop collapse persistence.

## Acceptance checks

- [x] Opening the drawer moves focus to a useful navigation control and keeps keyboard navigation within visible drawer controls.
- [x] Main content is inert and absent from the active accessibility interaction while the drawer is open.
- [x] Escape and the internal Close control close the drawer and restore opener focus.
- [x] Backdrop dismissal, route selection, and transition to desktop layout close the drawer correctly.
- [x] At 320, 390, and 768 CSS pixels, the opener does not intersect item counts, records, filters, forms, or primary actions at representative scroll positions.
- [x] Existing desktop collapse and navigation behavior remain unchanged.
- [x] Focused public Playwright keyboard, pointer, accessibility, and responsive checks pass.

## Dependencies and evidence

No ticket dependencies or backend changes. The audit confirmed that Escape did not close the drawer from the opener context, background content remained exposed, and the fixed opener covered content at 320 and 390 pixels. Full screen-reader qualification is not claimed from DOM checks alone.

## Comments

Chromium verified modal focus, keyboard and pointer dismissal, route and resize closure, desktop collapse persistence, and reflow at 320, 390, and 768 CSS pixels.
