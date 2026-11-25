# 13: Add truthful list discovery

**What to build:** Administrators can find and order Applications, Users, and Devices without copying UUIDs. Every control states whether it applies to loaded rows or the complete collection.

**Blocked by:** 04 — Repair Application creation and pagination

**Status:** ready-for-agent

- [x] Application, User, and Device lists provide keyboard-accessible search or filtering appropriate to their existing API capabilities.
- [x] Current-page controls are labelled as such and never imply server-wide search; server-wide controls are used only where the existing contract supports them.
- [x] Sorting and filtering have clear empty states and compose correctly with Application pagination and navigation.
- [x] Names remain linked, controls have accessible labels, and the workflows remain usable on small screens.
- [x] Focused state and real-backend browser tests cover filtering, sorting, empty results, pagination, and route navigation.

## Compatibility

Applications and Users filter and sort the current page. Devices filter and sort loaded Device records, not pending enrollments. Existing pagination, owner APIs, and stored data are unchanged. No migration is required.
