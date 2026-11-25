# 04: Repair Application creation and pagination

**What to build:** Administrators can create valid Applications, navigate by name, and paginate without malformed records or unlimited empty pages.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Malformed or unsafe URI values are rejected before an API request; valid values under the existing Application contract still work.
- [ ] Application names and IDs open the same detail destination.
- [ ] Next is disabled at the end of results and during errors; a short result page cannot advance to an empty offset.
- [ ] Failed page changes preserve the last valid rows and offset and show useful recovery guidance.
- [ ] Focused creation, update, deletion, URI, and pagination browser tests pass against the real backend.
