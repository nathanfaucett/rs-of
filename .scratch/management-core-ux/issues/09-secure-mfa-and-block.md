# 09: Make MFA and Block safe and functional

**What to build:** Administrators receive clear MFA and account-status results. Block requires confirmation, and failed requests never appear to change account state.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

- [x] Block confirmation identifies the User, and Cancel sends no request.
- [x] The owner HTTP boundary accepts exact-grant Block and Unblock and returns the updated account status.
- [ ] Browser success: authorized Block and Unblock update the displayed status only after service acknowledgement.
- [x] The owner HTTP boundary accepts exact-grant MFA enable and disable. The browser preserves prior state and gives useful guidance on denial.
- [ ] Browser success: authorized MFA enable and disable display the confirmed result.
- [x] Raw `ResponseError` text is replaced with an actionable message.
- [x] Real-backend tests use synthetic Users. Browser tests cover confirmation, denial, and failure without locking out the administrator; owner HTTP tests cover authorized success.

## Comments

- `account_security_requires_each_installation_permission` passed in the review worktree. It verified Block/Unblock and MFA enable/disable with separate exact grants at the owner HTTP boundary. `e2e/account-security.e2e.ts` covered Cancel, denial, and an interrupted MFA request with state preserved. Browser success remains unqualified; the MFA protocol did not change.
