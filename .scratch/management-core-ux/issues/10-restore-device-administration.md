# 10: Restore Device administration

**What to build:** An authorized administrator can rename a Device and manage its replication policy. Enrollment and timestamps provide clear, truthful feedback without the removed Sync panel.

**Blocked by:** 01 — Remove the dead Devices Sync panel

**Status:** ready-for-agent

- [ ] Authorized rename and policy read/update operations succeed through the existing contracts; unauthorized cases have clear recovery guidance.
- [ ] Policy mutations require confirmation, and denial or failure preserves the current state.
- [ ] Enrollment address guidance matches the existing Device contract, rejects malformed input locally where possible, and shows one actionable error while preserving valid drafts.
- [ ] Missing or unknown timestamps use an explicit fallback instead of the Unix epoch.
- [ ] Real-owner and real-backend browser tests cover rename, policy, enrollment validation, timestamps, and the absence of the Sync panel.

## Comments

The Device owner denies rename without the exact Installation grant and legacy owner binding. Replication policy read and update use separate grants; an acknowledged update does not require a follow-up read. Keep existing grants, owner checks, and the missing-policy response. No migration or API, persistence, or wire-format change is required.
