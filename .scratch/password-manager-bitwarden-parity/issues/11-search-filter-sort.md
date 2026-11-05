# 11: Search, filter, and sort unlocked items

**What to build:** Combined text search, type/favorite filtering, and clear sorting of the unlocked vault without a persistent plaintext index.

**Blocked by:** 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Search item names, usernames, websites, notes, and custom-field names from the unlocked in-memory view. Do not introduce persistent plaintext search caches or FTS tables.
- [ ] Combine text search with type and favorite filters and explicit sort controls. Define stable ordering and accessible empty/loading/error states.
- [ ] Keep results current after acknowledged create/edit/delete/refresh operations. Navigation to a result must use the existing detail flow without silently editing it.
- [ ] Search only available item types; additional type tickets extend coverage as they land. Folder filtering is delivered with folders; Trash integration must exclude deleted items from normal results.
- [ ] Test visible combined queries, clearing filters, Unicode, missing optional data, ordering, updates, and empty results using the real workflow/collection. Use representative synthetic large vaults without an invented timing guarantee.
- [ ] Clear decrypted query results on lock. Verify no plaintext index persists and all operations remain local/offline.

## Comments
