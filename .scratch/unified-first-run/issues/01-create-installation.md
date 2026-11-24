# 01: Create an Installation through Portless

**What to build:** Add an opt-in first-run path for the unified Node. On a fresh data root, an authorized operator uses the existing Installation setup flow to choose the first administrator username and password. Reuse protected setup admission and the owner setup lifecycle. Keep the current default startup unchanged in this slice.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] A fresh opt-in Node shows setup at the standard Portless URL instead of sign-in. No default administrator is seeded.
- [ ] Unauthorized setup writes fail. The app does not allow sign-in to the new Installation while setup is incomplete or owner readiness is unverified.
- [ ] An interrupted setup can resume without replacing credentials, identity, or retained state.
- [ ] A real-backend browser test creates an Installation, signs in with the entered credentials, verifies the old default credentials are rejected, and verifies that setup does not repeat after restart.
