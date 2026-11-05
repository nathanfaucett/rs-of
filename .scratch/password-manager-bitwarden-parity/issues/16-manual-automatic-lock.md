# 16: Enforce manual and automatic lock

**What to build:** Reliable manual/inactivity lock with persisted settings, retired vault handles, and no late return of decrypted state.

**Blocked by:** 02 — Authenticate empty and populated vaults; 03 — Separate vault identity from its name; 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Add accessible per-vault lock settings and persistent timeout behavior. Protect sensitive settings; document the supported timeout/restart semantics rather than assuming reference options.
- [ ] Use an inactivity deadline checked on activity and tab resume. A suspended timer must not leave an expired vault usable; do not claim ordinary browser code detects operating-system lock reliably.
- [ ] Manual and automatic lock retire the active workflow/collection and deny operations through the old vault handle. Release application references to keys, records, drafts, history, generated values, and displayed TOTP codes wherever those features exist.
- [ ] Define pending-write behavior. Late read/write/copy callbacks must not restore an unlocked view or report an unsaved change as successful. Preserve authoritative reconciliation where a write may have committed.
- [ ] Reopening requires correct authentication and reloads the saved settings. Lock must not erase stored items or prevent encrypted backup recovery.
- [ ] Test visible manual/automatic flows, persisted settings, tab suspension/resume, stale API handles with real persistence, active drafts, pending writes, and late callbacks. Extend checks as other features land.
- [ ] Keep lock local/offline. State JavaScript memory and clipboard limits honestly; do not add biometric, PIN, or client-side TOTP unlock claims.

## Comments
