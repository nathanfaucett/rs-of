# 03: Complete the Trash lifecycle

**What to build:** A person can restore a Trash item or deliberately delete it permanently through separate, clear actions. Confirmation, focus, labels, and dates match the Trash task.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

**Scope:** Trash row actions, permanent-delete confirmation, focus restoration, Trash-specific copy, and deletion-date presentation.

**Non-goals:** Automatic deletion, a 30-day retention policy, item-detail changes, backend changes, or Tombstone and synchronization changes.

- [x] Restore retains its current behavior.
- [x] Delete permanently uses the existing workflow and requires explicit confirmation naming the affected item.
- [x] Canceling either dialog returns focus to the action that opened it; successful deletion moves focus to a stable Trash destination.
- [x] A permanently deleted item remains absent after reload, while existing backups are not changed.
- [x] Trash does not show irrelevant New item or Search results controls and labels.
- [x] Deletion dates have localized display text and retain the exact timestamp programmatically.
- [x] Current no-automatic-deletion retention remains unchanged.
- [x] Public Playwright coverage proves restore, canceled permanent deletion, confirmed permanent deletion, and persistence after reload.

**Validation:** Run focused browser tests, then the Password Manager package test, type-check, lint, and build scripts. Run the Password Manager Playwright project before completion.

## Comments

- Implemented in `4c7a44eb`. The Chromium workflow verified restore, cancel and focus, confirmed permanent deletion, and persistence after reload. Backup format and retention were unchanged.
