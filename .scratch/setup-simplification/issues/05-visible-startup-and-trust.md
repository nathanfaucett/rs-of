# 05: Show startup immediately and request trust only when needed

**What to build:** Desktop users see a window and startup progress before slow host work. The Application explains necessary certificate trust consent, avoids unnecessary installer invocation when current trust is already satisfied, and handles startup failures and lifecycle actions safely.

**Blocked by:** 01 — Unify onboarding observation and recovery routing.

**Status:** resolved

- [x] Build the window before slow host preparation, certificate trust work, and HTTPS verification. Run startup asynchronously and expose progress/failure through the onboarding snapshot.
- [x] Verify current trust for the retained certificate before deciding installation is necessary. An old success marker cannot waive verification; unrelated startup errors must not be treated as permission to replace certificates or install new trust.
- [x] Explain the required OS trust change before requesting explicit installation/elevation consent. Already-trusted launches do not invoke installation unnecessarily.
- [x] Preserve retained host, port, CA, and server identity. Production clients retain native-root HTTPS verification, correct construction/refresh after trust changes, and existing Origin/admission protections.
- [x] Handle consent refusal, unavailable tools/provider prerequisites, verification failure, and explicit retry with actionable UI and retained evidence. No plaintext, custom production root bypass, or certificate-warning fallback is introduced.
- [x] Coordinate start, stop, retry, reset, and close through the same lifecycle ownership. Shutdown during startup cannot leak a listener, worker, or owned installer process or publish a stopped host as running.
- [x] A bundled-host startup failure does not prevent an otherwise usable selected remote-service workflow; bundled setup capabilities remain local.
- [x] Verify window/progress behavior at the existing rendered/native-adapter seam and lifecycle/trust behavior with existing isolated roots, injected commands, and protected-provider fixtures. Include already-trusted startup, refusal, failed verification, retry, and close/reset contention.
- [x] Keep native entry modules thin, delete obsolete synchronous startup/error paths, update platform prerequisite documentation, and record focused validation and its limits. Do not modify real OS trust or user profiles during automated tests.

## Answer

Implemented visible asynchronous native startup, conditional current native-root trust verification, explained explicit trust consent/refusal and retained-identity retry. Already-trusted launches skip installers; only typed unknown-issuer failures request installation. Verification clients refresh after consented changes; other startup errors never authorize identity replacement or trust mutation. One cancellable lifecycle owns startup/retry, close, consent and reset/recovery, joins workers/listeners/owned commands, and atomically rejects late host publication.

The minimal mounted setup integration polls startup, explains OS trust effects before Allow/Not now, handles refusal/retry and preserves remote selection independence. Removed obsolete synchronous startup cleanup; updated native/prerequisite/action/HTTP-inventory docs. Create/completion, Join and maintenance presentation remain owned by tickets 02/03/06.

[Focused evidence and qualification limits](../evidence/05-visible-startup-and-trust.md): 22 focused native tests, 15 rendered interactions, 181 state/route tests, app build/typecheck, changed-crate strict Clippy and formatting pass. The existing provider readiness bound also fails with original startup/verification restored; full matrix and CRAP time out compiling. No real OS trust/profile operations or native GUI/platform acceptance is claimed. Integration merge at `23e9006ca` was already up to date before commit.
