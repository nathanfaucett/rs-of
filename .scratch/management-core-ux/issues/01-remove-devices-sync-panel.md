# 01: Remove the dead Devices Sync panel

**What to build:** Devices contains only meaningful Device administration. The capability checker that always reports blocked transfers is removed without changing real synchronization.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

- [x] Devices has no Sync card, Configure sync action, Push/Pull/Bidirectional preferences, blocked target results, or `Sign in before checking sync.` message.
- [x] Device pairing, records, policy, Selection, and Residency controls remain available.
- [x] Code and documentation made unused by the removal are deleted; native Iroh commands and browser Storage synchronization are preserved.
- [x] The page does not read or write old sync-preference keys, and existing persisted entries are left untouched.
- [x] Focused real-backend browser tests and relevant existing synchronization checks pass.

## Comments

- `e2e/app.e2e.ts` confirmed panel removal, unchanged legacy preferences, and retained Device, Selection, and Residency controls. The vault sync and sync-client browser checks passed individually. Native Iroh code was unchanged; native transport was not requalified.
