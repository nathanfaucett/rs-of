# 03: Make Join a single guided request

**What to build:** A joining user supplies one trusted peer input, sees the authenticated target Installation, follows clear source-approval instructions, and reaches ordinary sign-in after the shared worker verifies completion.

**Blocked by:** 01 — Unify onboarding observation and recovery routing.

**Status:** ready-for-agent

- [ ] The bundled Join flow uses one trusted pinned-peer input and a default Node name rather than asking separately for issuer or internal configuration.
- [ ] Reuse authenticated issuer discovery and shared provisioning. Display authenticated target identity before membership finalization; do not trust arbitrary discovery, unsigned metadata, or unverified invitation claims.
- [ ] Present waiting for approval, rejection, provisioning progress, blocked prerequisites, and supported retry/cancellation actions without exposing owner phases as additional choices.
- [ ] Preserve the exact retained intent, peer, issuer, and identities across timeout, reload, restart, and authorized retry. Request transient credentials again when the existing contract requires them.
- [ ] Closing a page does not cancel accepted work; cancellation acknowledgement is not reported as completed rollback or cleanup. Do not imply automatic resume where explicit authorization is required.
- [ ] Proceed to ordinary sign-in only after current readiness and public login evidence verify. Preserve completed source-offline reopen.
- [ ] Keep advanced remote connection distinct from Node Join. Selected remote bases must not be replaced with bundled addresses or receive bundled setup capabilities, and connection alone must not provision bundled owners.
- [ ] Do not introduce invitation issuance, expiry, redemption, QR infrastructure, or a new pairing protocol. A presentation of existing evidence must preserve peer verification.
- [ ] Exercise visible Join behavior at the existing rendered-page seam and reuse public shared HTTPS Join tests for authenticated discovery, approval/rejection, retained retry identity, and offline reopen.
- [ ] Delete superseded Join UI and duplicate coordination, update current workflow documentation, and record focused validation outcomes. Approval can use existing source controls until ticket 04 lands.
