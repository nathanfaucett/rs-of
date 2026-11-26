# 04: Improve item details and website actions

**What to build:** Item details use available desktop space beside the list, remain usable in one column on narrow screens, show localized timestamps, and provide safe Launch actions for valid websites.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

## Scope

- Add a practical desktop list/details layout while preserving narrow-screen single-column behavior.
- Localize Created, Updated, and password-history timestamps using the existing Trash presentation policy and retain exact semantic datetime values.
- Add opener-isolated, new-context Launch actions only for valid HTTP(S) websites.
- Keep malformed or unsafe values as text with Copy only.

## Non-goals

- No broad visual redesign, new URI match rules, automatic navigation, or support for unsafe schemes.

## Acceptance checks

- [x] At 1024 and 1440 CSS pixels, users can inspect item details beside the list without losing list context.
- [x] Narrow layouts and the existing 200% layout-viewport check retain one column without horizontal overflow or clipped actions.
- [x] Item selection, details focus, Close details, Edit, Move to folder, and focus return work in both layouts.
- [x] Created, Updated, and password-history timestamps are localized, retain exact semantic datetime values, and handle missing or invalid values truthfully.
- [x] Valid HTTP(S) websites open in a new browsing context with opener isolation and an accessible action name.
- [x] Malformed and non-HTTP(S) values have no link and retain Copy Website and match-rule presentation.
- [x] Focused public Playwright layout, timestamp, link-safety, and existing item-workflow checks pass.

## Dependencies and evidence

No ticket dependencies, backend changes, or persisted-data migration. The audit observed details below the desktop list, raw ISO timestamps, and Copy-only website values. Browser-engine coverage beyond Chromium remains a qualification limit unless run.

## Comments

Chromium verified the desktop split, narrow layout and focus, localized semantic timestamps, and isolated Launch actions. Invalid and unsafe URLs remain copyable text.
