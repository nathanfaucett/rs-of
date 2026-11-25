# 03: Complete the Trash lifecycle

**What to build:** A person can restore a Trash item or deliberately delete it permanently through separate, clear actions. Confirmation, focus, labels, and dates match the Trash task.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** open

**Scope:** Trash row actions, permanent-delete confirmation, focus restoration, Trash-specific copy, and deletion-date presentation.

**Non-goals:** Automatic deletion, a 30-day retention policy, item-detail changes, backend changes, or Tombstone and synchronization changes.

- [ ] Restore retains its current behavior.
- [ ] Delete permanently uses the existing workflow and requires explicit confirmation naming the affected item.
- [ ] Canceling either dialog returns focus to the action that opened it; successful deletion moves focus to a stable Trash destination.
- [ ] A permanently deleted item remains absent after reload, while existing backups are not changed.
- [ ] Trash does not show irrelevant New item or Search results controls and labels.
- [ ] Deletion dates have localized display text and retain the exact timestamp programmatically.
- [ ] Current no-automatic-deletion retention remains unchanged.
- [ ] Public Playwright coverage proves restore, canceled permanent deletion, confirmed permanent deletion, and persistence after reload.

**Validation:** Run focused browser tests, then the Password Manager package test, type-check, lint, and build scripts. Run the Password Manager Playwright project before completion.
