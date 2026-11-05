# 13: Recover deleted items through Trash

**What to build:** Reversible deletion through a Trash view, with restore and separately confirmed permanent deletion.

**Blocked by:** 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Replace normal hard deletion with an encrypted persisted deletion state. Normal list/search/filter views exclude trashed items; provide a distinct accessible Trash view.
- [ ] Restore retains item identity, contents, history, and metadata. Define a safe unfiled fallback if an item's original folder no longer exists.
- [ ] Permanent deletion requires explicit separate confirmation and affects only the selected trashed item. Cancellation changes nothing.
- [ ] Do not add automatic purging based on an assumed retention period. Document that policy separately if automatic retention is later requested.
- [ ] Test delete, cancellation, Trash navigation, restore, permanent deletion, failed writes, and close/reopen through visible flows and real persistence. Extend coverage for each available item type.
- [ ] Include Trash state in encrypted backup/clean-storage restore coverage. Preserve authoritative-write verification and uncertain-outcome feedback.
- [ ] Keep deleted content encrypted, clear decrypted Trash state on lock, and preserve offline operation. Remove the obsolete normal hard-delete UI path.

## Comments
