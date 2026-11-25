# 01: Restore item browsing and folder assignment

**What to build:** A person can find an item at desktop or mobile size, open read-only details, reach TOTP and password history, move the item between Unfiled and an existing folder, and explicitly choose Edit. One search and one responsive filter presentation operate from the existing filter state.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** open

**Scope:** Item list, details, folder movement, responsive vault layout, New item menu, empty states, and user-facing Item terminology. Preserve current item types and literal folder semantics.

**Non-goals:** Nested folders, synchronization, backend changes, editor-safety repairs, and Trash lifecycle changes.

- [ ] Desktop shows one visible search input; search combines with the existing filters and Clear restores the full list.
- [ ] At 320, 390, 768, and 1024 pixels and 200% zoom, the item list remains reachable without horizontal page overflow, with one search input and one filter presentation.
- [ ] Item-name activation opens read-only details; explicit Edit opens the editor; Close returns focus to a sensible location.
- [ ] Existing TOTP codes and password history are reachable from Login details.
- [ ] Folder movement uses existing folder identities, preserves duplicate/literal-name behavior, and survives reload.
- [ ] The New item menu supports pointer use, arrow keys, Escape, outside dismissal, and focus return.
- [ ] Empty states offer clear New login and Import actions, and user-facing copy uses Item terminology consistently.
- [ ] The public Playwright workflow covers search, responsive layout, details, and folder movement using synthetic data and real browser persistence.

**Validation:** Run focused browser tests, then the Password Manager package test, type-check, lint, and build scripts. Run the Password Manager Playwright project before completion.
