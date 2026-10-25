# 05: Show startup immediately and request trust only when needed

**What to build:** Desktop users see a window and startup progress before slow host work. The Application explains necessary certificate trust consent, avoids unnecessary installer invocation when current trust is already satisfied, and handles startup failures and lifecycle actions safely.

**Blocked by:** 01 — Unify onboarding observation and recovery routing.

**Status:** ready-for-agent

- [ ] Build the window before slow host preparation, certificate trust work, and HTTPS verification. Run startup asynchronously and expose progress/failure through the onboarding snapshot.
- [ ] Verify current trust for the retained certificate before deciding installation is necessary. An old success marker cannot waive verification; unrelated startup errors must not be treated as permission to replace certificates or install new trust.
- [ ] Explain the required OS trust change before requesting explicit installation/elevation consent. Already-trusted launches do not invoke installation unnecessarily.
- [ ] Preserve retained host, port, CA, and server identity. Production clients retain native-root HTTPS verification, correct construction/refresh after trust changes, and existing Origin/admission protections.
- [ ] Handle consent refusal, unavailable tools/provider prerequisites, verification failure, and explicit retry with actionable UI and retained evidence. No plaintext, custom production root bypass, or certificate-warning fallback is introduced.
- [ ] Coordinate start, stop, retry, reset, and close through the same lifecycle ownership. Shutdown during startup cannot leak a listener, worker, or owned installer process or publish a stopped host as running.
- [ ] A bundled-host startup failure does not prevent an otherwise usable selected remote-service workflow; bundled setup capabilities remain local.
- [ ] Verify window/progress behavior at the existing rendered/native-adapter seam and lifecycle/trust behavior with existing isolated roots, injected commands, and protected-provider fixtures. Include already-trusted startup, refusal, failed verification, retry, and close/reset contention.
- [ ] Keep native entry modules thin, delete obsolete synchronous startup/error paths, update platform prerequisite documentation, and record focused validation and its limits. Do not modify real OS trust or user profiles during automated tests.
