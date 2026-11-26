# 10: Restore Device administration

**What to build:** An authorized administrator can rename a Device and manage its replication policy. Enrollment and timestamps provide clear, truthful feedback without the removed Sync panel.

**Blocked by:** 01 — Remove the dead Devices Sync panel

**Status:** ready-for-agent

**Work status:** resolved

- [x] Exact-grant rename and policy read/update succeed through the existing owner HTTP contracts. The browser gives clear recovery guidance on denial.
- [ ] Browser success: authorized Rename and policy read/update display the confirmed results.
- [x] Policy mutations require confirmation. Cancel sends no request, and denial preserves the current state.
- [ ] Browser qualification: a non-denial policy mutation failure preserves the current state.
- [x] Enrollment address guidance matches the existing Device contract, rejects malformed input locally where possible, and shows one actionable error while preserving valid drafts.
- [x] Missing or unknown timestamps use an explicit fallback instead of the Unix epoch.
- [x] Real-owner and real-backend browser tests cover rename, policy, enrollment validation, timestamps, and the absence of the Sync panel.

## Comments

- The focused Device owner HTTP tests passed exact-grant rename and separate policy read/update checks. `e2e/owners.e2e.ts` covered cancellation and denial, enrollment validation and draft retention, signed approval at the HTTP boundary, and an injected legacy timestamp fallback. Successful rename and policy operations were not verified through the browser.
