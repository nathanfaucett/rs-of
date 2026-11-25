# Password Manager core vault UX repair

Status: ready-for-agent
Work status: open
Milestone: M2 — local encrypted CRUD and lock/unlock

## Problem Statement

A person using the local encrypted vault cannot complete several basic password-manager workflows reliably:

- Desktop has no visible item search.
- The small-screen vault overflows horizontally and shows duplicate filters.
- Selecting an item opens its editor instead of the existing read-only details. This exposes secrets and makes folder assignment, current TOTP codes, and password history unavailable.
- Secret fields are visible by default in editors.
- A saved inactivity-lock change does not update the active session or remain visible after a route change.
- Cancel and navigation can discard an edited draft without warning.
- Trash offers Restore but no Permanent delete action.
- Creation, secret fields, menus, and clipboard actions have keyboard and accessibility defects.

These are defects in the reference application's existing frontend scope. They are not requests for broad Bitwarden feature parity.

## Solution

Repair the existing local-vault workflow so a person can:

1. Create and unlock a vault with normal form and keyboard behavior.
2. Find items with one persistent search and the existing filters at every supported viewport.
3. Open read-only item details, explicitly reveal or copy a secret, view TOTP and history, and assign a folder.
4. Choose Edit explicitly, with secrets masked by default and unsaved drafts protected.
5. Restore or permanently delete Trash items through clear, confirmed actions.
6. Change the inactivity timeout and have the confirmed value apply immediately, survive route changes, and use the existing persisted representation after unlock or reload.
7. Complete these workflows with keyboard navigation, clear accessible names, visible feedback, and a responsive single-column mobile layout.

Use the current vault, workflow, search, folder, lock, and persistence modules. Keep current item types, literal folder semantics, Trash retention, encrypted backup behavior, and local-first account-free operation.

## User Stories

1. As a person using a desktop vault, I want a visible search field above my items, so that I can find a login without changing viewport or opening filters.
2. As a person searching a vault, I want search to use the existing name, username, website, notes, and custom-field-name matching, so that behavior stays predictable.
3. As a person combining search with folder, type, and favorite filters, I want one shared filter state and a clear action, so that I can understand and reset the current result set.
4. As a mobile user, I want one column and one set of search/filter controls, so that I can reach my items without horizontal scrolling or duplicate controls.
5. As a person using a narrow window or zoomed text, I want item names, websites, buttons, and details to wrap safely, so that content and actions remain available.
6. As a person selecting an item name, I want read-only details, so that I can inspect an item without entering edit mode or exposing every secret immediately.
7. As a person viewing item details, I want explicit Edit and Close actions, so that the next step and return path are clear.
8. As a person viewing a Login, I want masked password and authenticator values with explicit reveal and copy actions, so that I control when sensitive values appear.
9. As a person using TOTP, I want the current code available from item details, so that the existing TOTP feature is reachable.
10. As a person reviewing a Login, I want password history available from item details, so that the existing history feature is reachable.
11. As a person organizing items, I want to move an item between Unfiled and an existing folder from its details, so that created folders have a usable purpose.
12. As a person with duplicate literal folder names, I want each folder to retain its stable identity without exposing raw identifiers as the primary label, so that I can distinguish destinations only when necessary.
13. As a person editing any sensitive field, I want it masked by default, so that passwords, card values, identity numbers, private keys, TOTP keys, and hidden custom fields are not exposed on entry.
14. As a person revealing a field, I want the control to say whether it will Show or Hide the value, so that its action and state are clear to sighted and assistive-technology users.
15. As a person copying a value, I want success or failure announced without showing the value or internal exception, so that I know whether to paste or choose another action.
16. As a person editing a draft, I want Cancel, Back, sidebar navigation, browser Back, and reload to warn only when changes are unsaved, so that I do not lose work accidentally or receive needless prompts.
17. As a person who just saved an item, I want to return to the vault without a dirty-draft prompt, so that successful completion is not treated as abandonment.
18. As a person using Trash, I want Restore and Delete permanently to be separate actions, so that I can recover an item or deliberately remove it.
19. As a person permanently deleting an item, I want a clear confirmation and predictable focus after the dialog, so that I do not destroy the wrong item or lose my place.
20. As a person changing the inactivity timeout, I want the confirmed setting to apply to the current vault session immediately, so that the displayed security behavior is the behavior actually used.
21. As a person returning to Settings or unlocking again, I want the confirmed timeout to remain selected, so that the UI reflects retained vault state.
22. As a person creating or unlocking a vault, I want Enter to submit the active form and busy states to prevent duplicate submission, so that keyboard use is reliable.
23. As a person creating a master password, I want the current minimum-length rule visible before submission, so that I can satisfy it without a preventable error.
24. As a keyboard user, I want the New item menu to support arrow keys, Escape, outside dismissal, and focus return, so that I do not need a pointer.
25. As an assistive-technology user, I want each input's accessible name to contain its field label rather than adjacent button text, so that I can identify the correct control.
26. As a person switching routes, I want a useful document title, so that browser history and tabs identify the current Password Manager view.
27. As a new vault user, I want an empty state with clear Add first login and Import actions, so that my next step is obvious.
28. As a person reading item and Trash views, I want view-specific labels and actions rather than irrelevant Search results or New item controls, so that the interface matches the current task.
29. As a person reading deletion dates, I want a localized date with the exact timestamp still available, so that I can understand when an item was deleted.
30. As a person using the reference application, I want consistent Item terminology for logins, notes, cards, identities, and SSH keys, so that the UI does not use developer-oriented Secret language unnecessarily.

## Implementation Decisions

### Navigation and item details

- Use the existing local-secret workflow as the owner of the open item, detail, editor, folder, and Trash states.
- Item-name activation opens the existing read-only detail view. The explicit pencil action opens the editor. Do not create a second item-selection model.
- Keep the existing detail component as the single read-only presentation for fields, TOTP, password history, and folder movement.
- Closing details restores focus to the originating item when it still exists; otherwise use a stable fallback such as the item-list heading.
- Folder movement uses existing folder IDs and workflows. Literal names, duplicate names, slashes, and Unfiled behavior remain unchanged.
- Use **Item**, **Items**, and **New item** in user-facing copy. Do not rename internal secret models or persisted kinds as part of this UI change.

### Search, filters, and responsive layout

- Reuse the existing controlled filter state and item-search function. Do not add a search library or another filtering implementation.
- Place one visible search control with the item-list header at every viewport. Keep its current search semantics.
- Retain the desktop filter sidebar and compact mobile filters as alternate presentations of the same state, not simultaneous controls.
- Below the desktop breakpoint, use a single-column layout with only compact filters. At the desktop breakpoint and above, use the sidebar layout without compact filters.
- Remove horizontal overflow at 320, 390, 768, and 1024 CSS pixels and at 200% zoom. Allow long URIs, names, and secret values to wrap without exposing their contents by default.
- Keep the existing navigation drawer and Sync Status behavior. Layout changes must not introduce a second navigation model.
- Empty results distinguish an empty vault, no search/filter matches, and an empty folder. The empty-vault state links to New login and the existing import workflow.

### Secret display and clipboard feedback

- Reuse the shared password-input and protected-value components for all sensitive field types.
- Sensitive editor fields start masked on every new component instance. Reveal applies only to that field instance.
- Reveal controls use state-specific accessible names and pressed state. Description and error IDs are unique per field instance.
- Preserve the existing field-control interface while separating labels from groups of input and action buttons. Adjacent actions must not become part of the input's accessible name.
- Clipboard completion exposes only a short success or failure status. Never include the copied value, exception text, or secret field content in notifications or logs.
- Reuse the generator's existing denied-clipboard feedback behavior where practical.

### Dirty-draft protection

- Use the existing workflow dirty state as the single source for whether a draft needs protection.
- Guard Cancel, explicit Back, application navigation, browser Back, and page unload from the editor boundary. Do not add independent dirty flags to each route or field.
- Prompt only for a dirty, unsaved draft. Canceling the prompt keeps the current draft intact.
- A clean draft, canceled draft after confirmation, and successful save navigate without another prompt.
- Do not block an in-progress save or allow a late save callback to reopen an editor after the vault locks.

### Trash

- Add a distinct Delete permanently action to each Trash row and connect it to the existing permanent-delete workflow and confirmation dialog.
- Keep Restore behavior and current no-automatic-deletion retention unchanged.
- Trash does not show New item or search-only labels. Deletion dates use localized display text while retaining the exact timestamp programmatically.
- Confirmation cancel returns focus to the action that opened the dialog. Successful deletion moves focus to a stable Trash destination.

### Inactivity-lock state

- Make the unlocked vault-state owner responsible for saving and applying lock settings. The Settings component requests a save through that public application-state boundary; it does not mutate a derived data object.
- The state operation persists through the existing vault interface, uses the confirmed returned settings, updates the owning unlocked state, and reapplies those settings to the active inactivity controller.
- Saving is user activity and starts a new inactivity deadline from the confirmed save.
- Route remounts read the owning unlocked state. Unlock and reload continue to load the existing persisted settings.
- A late completion after lock or a newer vault session must not update the new session. Use the existing session/close ownership rules.
- Failure retains the previous active and displayed settings and produces actionable feedback. Do not report success before persistence and active-state confirmation.

### Forms, keyboard, and browser polish

- Vault creation and unlock use native form submission, including Enter, existing validation, disabled busy states, and appropriate current/new-password autocomplete semantics.
- Show the existing master-password minimum as persistent field help. Do not change the minimum in this repair.
- Complete the existing New item menu keyboard behavior without adding a dependency. Reuse an existing shared primitive if it already supplies the full contract.
- Add route-specific document titles. Replace the framework-default favicon with a simple application icon; do not create a new brand system.
- Remove duplicate route/card headings where both describe the same task. Keep one clear heading and one useful description.

### Compatibility and ownership

- No persisted-data schema, encrypted-record format, public HTTP API, generated client, or wire-protocol change is required or permitted for this repair.
- Existing vaults, items, folders, Trash records, and lock settings need no migration.
- Keep fixes in the Password Manager frontend and its existing state owner. Do not move domain ownership into services or add a synchronization path.
- Update current application documentation and affected tests with behavior changes. Do not claim production password-manager qualification.

## Testing Decisions

### Primary seam: public browser workflow

Use the existing Password Manager Playwright project as the highest and primary seam. Drive the public UI with synthetic secrets and the real browser vault persistence. Do not mount a replacement workflow, bypass the UI, inspect private component state, or assert implementation-specific class names.

The existing project already covers local-vault creation and responsive navigation. Extend that coverage with focused local-only scenarios; no sign-in or synchronization assertion is needed.

### Supporting seam: existing Vitest browser components

Use the existing Vitest Browser Mode and Svelte component-mount pattern for narrow boundaries that are costly to exercise repeatedly through full routes:

- Unique field descriptions and exact accessible names.
- Mask/reveal state and clipboard success/failure without secret leakage.
- Lock-setting confirmation, failure, and stale-session handling at the application-state boundary.

Do not duplicate every browser workflow in component tests. External behavior, not internal method calls, remains the acceptance target.

### Acceptance checks

1. Vault creation submits with Enter, retains its current validation and busy behavior, and shows the password rule before submission.
2. At desktop width, one search input is visible. Search and existing filters return the expected synthetic items, and Clear restores the unfiltered list.
3. At 320, 390, 768, and 1024 pixels and 200% zoom, the vault has no horizontal page overflow, the item list is reachable, and only one filter presentation and one search input are visible.
4. Selecting an item name opens read-only details. The explicit Edit action opens the editor, and Close restores sensible focus.
5. Login details expose the existing TOTP and password-history features. Folder movement changes the item's folder and survives a reload.
6. Passwords and all other protected field kinds start masked in details and editors. Reveal affects only the chosen field.
7. Exact field labels resolve to their intended inputs. No duplicate description IDs or action-button text pollutes those accessible names.
8. Clipboard success and denied permission both produce non-secret feedback. Test exception text and copied values never appear in the feedback.
9. Dirty Cancel, Back, sidebar navigation, browser Back, and reload require confirmation. Clean navigation and successful save do not prompt.
10. Trash restores one item and permanently deletes another through separate confirmed flows. Deleted items remain absent after reload; current retention behavior is unchanged.
11. Saving a new timeout applies it to the active controller, remains selected after leaving and returning to Settings, and survives lock/unlock and reload. Use Playwright clock control for the one-minute active-lock boundary after vault setup rather than adding a real-minute test delay.
12. A failed or stale lock-setting save leaves the previous session settings intact and does not report success.
13. The New item menu works with pointer and keyboard, closes on Escape and outside activation, and returns focus to its trigger.
14. Item and Trash views use view-appropriate labels and actions, and tested routes have meaningful document titles.
15. Existing local-vault CRUD, wrong-password unlock, manual lock, import/export, and backup tests remain green.

### Required validation

- Run the focused Password Manager tests while working.
- Before completion, run the Password Manager package test, type-check, lint, and build scripts.
- Run the Password Manager Playwright project for final browser qualification. Record any unrun browser or synchronization checks explicitly.
- Chromium is the required browser for this repair. Do not claim Firefox, WebKit, screen-reader, real-device, or production security qualification without separate evidence.

## Out of Scope

- Synchronization, OIDC, Storage, Device, Selection, Installation, and backend changes.
- Nested folder semantics or changes to duplicate/literal folder behavior.
- Automatic Trash retention or Bitwarden's 30-day Trash policy.
- Master-password policy changes, strength meters, breach checks, master-password change, biometric unlock, or per-item master-password re-prompt.
- Generator history, additional generator controls, or a new generator workflow.
- New import/export formats, attachment support, plaintext-export re-prompt, or changes to encrypted-backup format.
- Bulk actions, clone, archive, passkeys, Send, organizations, sharing, collections, or native autofill.
- A general visual redesign, theme system, or new product brand.
- Production password-manager qualification or full Bitwarden feature parity.

## Further Notes

- Manual Chromium evidence on 2026-11-25 reproduced the principal defects with a synthetic local vault at desktop and mobile sizes.
- Existing useful behavior should be preserved: five item types, TOTP, URI match rules, custom fields, favorites, password/passphrase generation, wrong-password guidance, manual lock, delete confirmation, Trash restore, export preview, and encrypted-backup recovery.
- Bitwarden documentation supplied a comparison baseline only. No authenticated Bitwarden UI session was used.
- Cross-restart lock-setting persistence was not qualified during the review and is therefore an explicit acceptance check here.
- This work advances the local encrypted CRUD and lock/unlock portions of M2. It does not qualify M2 synchronization or broader milestones.
