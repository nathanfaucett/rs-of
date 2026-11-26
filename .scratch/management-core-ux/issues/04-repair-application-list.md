# 04: Repair Application creation and pagination

**What to build:** Administrators can create valid Applications, navigate by name, and paginate without malformed records or unlimited empty pages.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

- [x] Malformed or unsafe URI values are rejected before an API request; valid values under the existing Application contract still work.
- [x] Application names and IDs open the same detail destination.
- [x] Next is disabled at the end of results and during errors; a short result page cannot advance to an empty offset.
- [x] Failed page changes preserve the last valid rows and offset and show useful recovery guidance.
- [x] Focused creation, update, deletion, URI, and pagination browser tests pass against the real backend.

## Comments

- `e2e/applications.e2e.ts` confirmed URI rejection before submission, valid create/edit/delete, name and ID links, and exact and short page boundaries. An interrupted page request kept the prior offset and rows.
