# 12: Organize items in encrypted folders

**What to build:** Vault-local encrypted folders with create/rename/delete, item movement, and a folder-filtered view that never deletes contained items by mistake.

**Blocked by:** 01 — Verify the reference feature baseline; 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Persist folders and item assignments encrypted at rest. Provide accessible folder navigation, create, rename, move, unfile, and filter actions through the application.
- [ ] Use verified naming/nesting semantics. Do not claim hierarchical parity before evidence supports it; do not add organizations or shared collections.
- [ ] Confirm folder deletion and leave contained items accessible as unfiled items. Failed move/delete operations must not produce orphaned or lost records.
- [ ] If multiple records must change together, use an atomic or explicitly recoverable persistence operation rather than treating a loop of single-item writes as atomic.
- [ ] Folder filtering works independently and composes with search/type/favorite controls when those controls are available, regardless of ticket landing order. Cover empty and unfiled views.
- [ ] Test user-visible workflows, cancellation, name collisions, failed writes, lock/reopen, and vault isolation. Extend backup/clean-storage restore tests for folders and assignments.
- [ ] Preserve complete-snapshot validation, authoritative-write verification, lock cleanup, and offline operation.

## Comments
