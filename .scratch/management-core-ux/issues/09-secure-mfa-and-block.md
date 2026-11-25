# 09: Make MFA and Block safe and functional

**What to build:** Administrators receive clear MFA and account-status results. Block requires confirmation, and failed requests never appear to change account state.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Block confirmation identifies the User, and Cancel sends no request.
- [ ] Authorized Block and Unblock requests update the displayed account status only after service acknowledgement.
- [ ] MFA enable and disable results are visible, and denial preserves the previous state with useful guidance.
- [ ] Raw `ResponseError` text is replaced with an actionable message.
- [ ] Real-backend tests use synthetic Users and cover confirmation, success where authorized, denial, and failure without locking out the administrator.
