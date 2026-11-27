# 03: Restore a remote-only database into a fresh browser

**What to build:** When the signed-in namespace already has a Password Manager Resource and the browser has no local data, reconcile that Resource into the active local database. Show its vaults in the normal selector so the User can unlock and use them without the legacy synced-vault path.

**Blocked by:** 02 — Attach an existing local database to a new Storage Resource

**Status:** ready-for-agent

Work status: resolved

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- Preserve the existing Resource ID and establish the local copy as a replica of that Resource.
- Keep local unlock and vault IDs unchanged.
- Do not treat an empty browser as permission to merge or replace populated local data.
- Do not add two-Device convergence or continuous delivery claims.

## Acceptance checks

- [x] A fresh browser with no local vault data receives the selected remote database through the authorized SQL transport.
- [x] Remote vaults appear in the normal selector and unlock with their existing master passwords.
- [x] Attachment does not create another Resource or require the legacy synced-vault selector.
- [x] After the initial exchange, the User can reload offline and unlock and edit the retained local copy.
- [x] Reopening the same account reuses the same Resource and does not duplicate vaults.
- [x] If both sides contain independently created data, the app stops at an explicit merge-required state without changing either side.
- [x] The behavior passes against a real, non-seeded unified backend.

## Current evidence and qualification limits

Fresh non-seeded Chromium passed restore by exact creation key, reload, offline unlock, and offline edit. A competing browser write prevents publication. Pull requires existing Resource history and read-only authorization. Cold standalone Pull refuses to open a Redb file; unified Storage does not have this limit. Full OpenAPI qualification timed out; focused Storage client tests passed.
