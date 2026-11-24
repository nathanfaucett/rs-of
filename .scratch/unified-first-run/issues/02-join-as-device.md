# 02: Join an existing Installation as a Device

**What to build:** Add an opt-in join flow for a fresh instance. The user selects the existing Installation’s HTTPS services. Reuse its Device pairing contract to verify possession of the joining Device’s endpoint key and obtain approval from an authorized user. Use an existing Installation fixture so this slice can run without ticket 01.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] The join flow uses the existing Installation’s accounts. It creates no local IdP, administrator, or second server authority.
- [ ] Sign-in alone does not approve the Device. Approval requires the pairing proof and authorization. Device approval does not grant Resource access or Selection.
- [ ] A real-backend browser test verifies pending and approved states and confirms that the same approved Device identity persists across restart.
