# 03: Repair modal identity and dismissal

**What to build:** Every management dialog has its own accessible name and working dismissal controls, including when several modals are mounted on one page.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Modal title IDs are unique, and Configuration, Create User, Delete User, and Reset Password expose the correct accessible names.
- [ ] Cancel closes Delete User and Reset Password; existing Cancel, Close, and Escape behavior remains working elsewhere.
- [ ] Focus enters the dialog and returns to the invoking control after dismissal.
- [ ] Keyboard and accessibility-focused browser checks cover multiple mounted modals and pass.
