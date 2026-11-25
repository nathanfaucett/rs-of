# 08: Repair password reset

**What to build:** Password reset requires a real destination and reports its result in the dialog. Users without a supported destination receive an explanation instead of a failing action.

**Blocked by:** 03 — Repair modal identity and dismissal

**Status:** ready-for-agent

- [ ] A User without an email or supported reset destination cannot submit and sees the reason; no placeholder destination appears.
- [ ] The dialog shows the actual destination when one exists and reports local request success or failure without leaving an ambiguous state.
- [ ] Service `500` responses become useful guidance, and Cancel closes the dialog before or after failure.
- [ ] Real-owner and browser tests cover missing destination, successful local reset behavior, service failure, and dismissal without claiming email delivery.
