# 07: Integration: End-to-End Device Approval Flow with Full Three-Layer Enforcement

**What to build:** Complete end-to-end flow: User signs in via OIDC (IdP) → Admin presents Management permission → Device approval requires all three layers (policy eval + active user check + state mutation) → Device state changes from Pending to Approved → Selection policies update accordingly.

**Blocked by:** 01, 02, 03, 04, 05, 06 (all prior tickets must complete first)

**Status:** ready-for-agent

**Acceptance criteria:**
- [ ] End-to-end tests exercise the full device approval flow with all three layers enforced
- [ ] Each layer can independently reject the flow (policy fail, IdP fail, state fail)
- [ ] All three layers passing succeeds the flow and updates device state + selection policies
- [ ] The flow is verified through the public API surface (unified-server routes)

**Blocked by:** 01, 02, 03, 04, 05, 06
