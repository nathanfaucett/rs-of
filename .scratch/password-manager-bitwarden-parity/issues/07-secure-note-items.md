# 07: Store Secure Note items

**What to build:** Complete encrypted Secure Note creation, reading, editing, cancellation, and reopen without a required login password.

**Blocked by:** 05 — Transition existing records to typed Login items; 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Add Secure Note selection and type-specific validation through the item model, persistence, list, detail, and editor. Start with private plain text; rich text and Markdown preview are not required.
- [ ] Preserve applicable notes, favorites, custom fields, identity, and timestamps using the verified field mapping. Do not synthesize a password to satisfy the former Login model.
- [ ] Create, view, edit, cancel, save, and reopen a note with Unicode and multiline content. Editing or cancelling one item must not change another.
- [ ] Retain encrypted-at-rest storage, complete-snapshot validation, acknowledged writes, protected detail actions, keyboard access, and lock cleanup.
- [ ] Extend encrypted backup/clean-storage restore coverage for notes and mixed Login/Note vaults. Test visible workflows and real-persistence reopen with failure cases.
- [ ] Preserve offline operation and include any required assets in offline delivery.

## Comments
