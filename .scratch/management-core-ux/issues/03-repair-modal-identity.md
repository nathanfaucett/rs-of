# 03: Repair modal identity and dismissal

**What to build:** Every management dialog has its own accessible name and working dismissal controls, including when several modals are mounted on one page.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

- [x] Modal title IDs are unique, and Configuration, Create User, Delete User, and Reset Password expose the correct accessible names.
- [x] Cancel closes Delete User and Reset Password; existing Cancel, Close, and Escape behavior remains working elsewhere.
- [x] Focus enters the dialog and returns to the invoking control after dismissal.
- [x] Keyboard and accessibility-focused browser checks cover multiple mounted modals and pass.

## Comments

- `e2e/management-modals.e2e.ts` confirmed distinct names and IDs, Cancel, Close, Escape, and focus return in Chromium. Screen readers were not tested.
