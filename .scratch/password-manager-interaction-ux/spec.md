# Password Manager editing, import, and navigation repair

**Status:** ready-for-agent

**Milestone:** M2 — Single-Node encrypted synchronization

## Problem Statement

The local Password Manager supports core vault workflows, but several interactions make those workflows unnecessarily difficult or inaccessible. Identity and Secure Note creation hide ordinary fields behind Show controls. Card and Identity editing emit Svelte prop-ownership warnings. Import Cancel removes the only import entry point, and a successful import leaves a repeat action active. Mobile navigation obscures content and does not manage keyboard focus or Escape correctly. Item details add avoidable scrolling, expose raw timestamps, and lack a safe website-launch action.

These are frontend interaction defects. They do not require a new vault, synchronization, persistence, or authorization model.

## Solution

Make ordinary item data immediately editable, keep actual secrets protected, and remove editor prop-ownership warnings. Make Import cancel and completion return to clear, safe states. Make mobile navigation keyboard-accessible and non-obscuring. Improve item-details presentation with a practical desktop layout, localized timestamps, and safe website launching.

Preserve the existing encrypted local vault, public item contracts, masked secret display, clipboard behavior, import batch guarantees, and current synchronization implementation.

## User Stories

1. As a user creating an Identity, I want ordinary fields such as names, email, phone, and address visible immediately, so that I can enter an Identity without revealing every field separately.
2. As a user editing an Identity, I want sensitive fields to remain masked where appropriate, so that ordinary data entry does not weaken secret protection.
3. As a user creating a Secure Note, I want the note editor visible immediately, so that I can enter the main content without an extra Show action.
4. As a user editing secrets, I want masked inputs and independent reveal controls to keep working, so that my passwords, private keys, and hidden custom fields remain protected.
5. As a maintainer, I want Card and Identity draft ownership declared correctly, so that normal editing produces no Svelte prop-ownership warnings or unreliable reactivity.
6. As a user correcting an invalid import, I want Cancel to return to the file picker, so that I can select a corrected file without leaving Settings.
7. As a keyboard user, I want Import cancel, validation, and completion to move focus to a useful next action, so that I know where to continue.
8. As a user who completed an import, I want the committed preview retired, so that I cannot accidentally submit the same preview again.
9. As a user starting another import, I want an explicit new-file action after completion, so that I can begin a separate batch with clear state.
10. As a user importing records, I want existing items, Trash, folders, and lock settings preserved, so that interface repairs do not change import data guarantees.
11. As a mobile keyboard user, I want focus to enter the navigation drawer when it opens, so that I can use its controls immediately.
12. As a mobile keyboard user, I want Escape and an explicit Close control to close the drawer and return focus to the opener, so that I can leave it predictably.
13. As a screen-reader user, I want background content excluded while the modal drawer is open, so that hidden page controls do not compete with navigation.
14. As a mobile user, I want the navigation opener to avoid records and actions, so that I can use Sort, item counts, and controls at every scroll position.
15. As a desktop user, I want item details beside the list when space permits, so that I can inspect records without losing list context or scrolling through a long page.
16. As a narrow-screen user, I want the list and details to remain usable in one column, so that the desktop improvement does not create overflow or clipped actions.
17. As a user reviewing an item, I want localized Created, Updated, and password-history timestamps, so that I can understand chronology without parsing raw ISO text.
18. As a user reviewing a Login, I want a Launch action for a valid website, so that I can open the sign-in page without copying its URI manually.
19. As a security-conscious user, I want malformed or unsafe website values excluded from links, so that stored content cannot create an unsafe navigation action.
20. As a user relying on existing vault behavior, I want dirty-draft protection, folder movement, filtering, clipboard feedback, and lock behavior preserved, so that this repair does not regress established workflows.
21. As a user with an existing local vault, I want my records and settings to open without migration, so that a frontend repair does not change persisted data.
22. As a maintainer, I want browser tests to reject new runtime errors and editor ownership warnings, so that fixed defects do not return unnoticed.

## Implementation Decisions

### Item editor and protected values

- Separate ordinary text-entry presentation from protected-value presentation at the existing item-editor boundary.
- Render ordinary Identity fields and Secure Note or Identity notes as visible inputs or textareas during creation and editing. Do not require a reveal action before the first entry.
- Keep passwords, authenticator keys, private keys, payment secrets, government identifiers, and hidden custom fields masked by default. Their editable inputs must remain available, and their independent Show, Hide, and Copy behavior must remain intact.
- Keep saved-item details masked by default for every protected value. This change affects editor presentation, not the existing detail-protection contract.
- Declare draft ownership explicitly between the item editor and the Card and Identity field components. Use the existing bindable draft pattern; do not add a parallel draft store or callback protocol.
- Preserve dirty-draft detection, native validation, Enter submission, generator insertion, custom-field behavior, and encrypted save behavior.
- Do not silence warnings by suppressing Svelte diagnostics. Fill every Card and Identity field during browser qualification and verify that ownership warnings are absent.

### Import lifecycle

- Treat file selection, validation preview, busy commit, successful completion, and cancellation as explicit interface states.
- Cancel must retire only the active file or preview and return to the visible file picker. It must not retire the mounted Import feature itself.
- After a validation failure, allow a corrected file to be selected on the same Settings visit. Preserve all committed vault data.
- After successful commit, retire the committed preview and disable its old commit action. Show the verified result and an explicit action to start another import.
- Starting another import must clear the old file, preview, confirmation, and transient errors before offering a new picker.
- Move focus to the file picker after Cancel, the verified result after success, and the new picker when starting another batch. Remove stale focus-target assumptions.
- Preserve the existing complete-batch commit, duplicate confirmation, source-ID handling, new-local-identity behavior, and uncertain-write recovery guidance.
- Do not add skip, merge, overwrite, automatic duplicate removal, or partial supported-only import.

### Mobile navigation

- Treat the small-screen drawer as a modal navigation surface while it is open.
- Move focus to the current navigation item or another useful first control on open. Keep keyboard navigation within visible drawer controls and make the main content inert until close.
- Provide an explicit Close control inside the drawer. Preserve the backdrop as a pointer-dismiss path, not as the only keyboard-close mechanism.
- Close on Escape from any drawer control, route selection, and transition to the desktop layout. Return focus to the opener after Escape or Close.
- Preserve existing arrow, Home, and End navigation behavior.
- Move the opener into a non-obscuring mobile header or reserve equivalent layout space. It must not cover list content, item counts, filters, forms, or actions at supported widths and scroll positions.
- Keep the existing desktop collapse preference and persistence behavior unchanged.

### Item details and website actions

- Use a list/details split at desktop widths where there is enough space. Retain a single-column presentation at narrow widths.
- Preserve item selection, details focus, Close details, Edit, Move to folder, and focus return behavior in both layouts.
- Format Created, Updated, and password-history timestamps with the same localization policy used by Trash. Keep the exact instant in the semantic time element and show a truthful fallback for missing or invalid values.
- Add a Launch action beside each valid HTTP or HTTPS website value. Open a new browsing context with opener isolation and an accessible name that identifies the website.
- Render malformed values and non-HTTP(S) schemes as text with Copy only. Do not interpolate stored values into unsafe link destinations.
- Keep Copy Website behavior and URI match-rule presentation unchanged.

### Compatibility and ownership

- Make the smallest complete change in the owning Password Manager frontend modules. Reuse existing field, protected-value, import, navigation, and link primitives before adding abstractions.
- No schema, persistence-format, public API, authorization, or wire-protocol change is expected. Any evidence requiring one must be recorded before implementation continues.
- Existing vault records, folders, Trash, imports, backups, and lock settings require no migration.
- Do not change synchronization, OIDC, Storage, or native Iroh behavior.
- Do not add a fictitious username field solely to silence Chromium's verbose autofill guidance. Retain correct current-password and new-password metadata; external password-manager integration is not part of this repair.

## Testing Decisions

### Primary seam: existing Password Manager Playwright project

Use the existing public-browser Password Manager Playwright project as the primary seam. Exercise real IndexedDB vault behavior through visible controls rather than component internals.

Cover:

- New and edited Identity records expose ordinary inputs immediately, while defined secret fields remain masked and independently revealable.
- New and edited Secure Notes expose the note editor immediately. Notes save, reopen, and display with the intended protection.
- Every Card and Identity field accepts and persists edits without `ownership_invalid_mutation` warnings.
- Invalid import, Cancel, corrected-file selection on the same Settings visit, successful commit, retired preview, and explicit new-file start.
- Exact record counts after import and repeated clicks around completion, proving that a committed preview cannot be committed twice.
- Preservation of existing items, Trash, folders, duplicate handling, and lock settings through Cancel and successful import.
- Focus after import Cancel, success, and new-file start.
- Drawer focus entry, contained keyboard navigation, inert background, Escape, explicit Close, pointer dismissal, route selection, resize close, and opener focus return.
- Navigation-opener intersection with content and primary actions at 320, 390, and 768 CSS pixels, including representative scrolled positions.
- Desktop list/details layout at 1024 and 1440 CSS pixels and single-column behavior below the desktop breakpoint. Include the existing 200% layout-viewport check.
- Localized visible timestamps with exact semantic datetime values.
- Launch for valid HTTP(S) websites and absence of links for malformed or unsafe schemes.
- Existing dirty-draft, secret display, clipboard, lock, folder, search, filter, sort, and Trash workflows relevant to changed components.
- No new browser console errors and no Card or Identity prop-ownership warnings. Exclude only identified development-server and tooling messages.

Use synthetic records and isolated browser vaults. Do not use real credentials or change administrator fixtures.

### Supporting seam: existing Vitest component and state tests

Use focused existing Vitest seams for protected-field presentation and import state transitions where they give precise coverage that is inefficient in the browser. Keep public workflow acceptance in Playwright.

Update the current masked-editor tests that intentionally expect ordinary notes to be absent before Show. Replace that outdated expectation with separate ordinary-editor and protected-secret contracts. Do not weaken assertions for private keys, hidden custom fields, passwords, or saved-item details.

### Acceptance evidence and final validation

For focused work, run the smallest relevant Playwright and Vitest selections. Final validation must include the Password Manager package tests, type check, lint, build, E2E TypeScript check, and full Password Manager Playwright project.

Record any unrun browser engines, real mobile devices, screen-reader qualification, external password-manager autofill, or synchronization checks as qualification limits. Never claim an unrun check passed.

## Out of Scope

- Production password-manager parity or a broad visual redesign.
- Vault rename/delete, master-password rotation, biometric or PIN unlock, and clipboard auto-clear settings.
- Attachments, passkeys, Send, organizations, collections, sharing, nested folders, or new Resource types.
- Username generation, generator history, SSH key generation, and SSH agent integration.
- New import formats, merge/overwrite strategies, automatic duplicate removal, or changes to import persistence guarantees.
- Synchronization, OIDC, Storage, authorization, backend services, and native desktop packaging.
- Firefox, WebKit, real mobile-device, and full screen-reader qualification beyond the checks actually run.
- External password-manager autofill integration or changes designed only to suppress browser verbose messages.

## Further Notes

The source evidence is a Playwriter walkthrough on 2026-11-25 against the local Password Manager at desktop and mobile sizes. It exercised vault creation and locking, all five item types, folders, search and filters, secret display and clipboard feedback, import/export, Trash, and responsive navigation.

The audit confirmed working core CRUD and found no HTTP failures in the stable session. Vite/HMR and audit-tool errors were excluded. The six synthetic audit items and synthetic folder were removed, and the isolated browser sessions were deleted.

The accepted scope is a focused interaction repair. Broader Bitwarden capability gaps remain recorded in the temporary audit evidence and are not requirements for this specification.
