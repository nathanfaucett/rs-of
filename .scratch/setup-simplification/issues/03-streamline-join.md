# 03: Make Join a single guided request

**What to build:** A joining user supplies one trusted peer input, sees the authenticated target Installation, follows clear source-approval instructions, and reaches ordinary sign-in after the shared worker verifies completion.

**Blocked by:** 01 — Unify onboarding observation and recovery routing.

**Status:** resolved

- [x] The bundled Join flow uses one trusted pinned-peer input and a default Node name rather than asking separately for issuer or internal configuration.
- [x] Reuse authenticated issuer discovery and shared provisioning. Display authenticated target identity before membership finalization; do not trust arbitrary discovery, unsigned metadata, or unverified invitation claims.
- [x] Present waiting for approval, rejection, provisioning progress, blocked prerequisites, and supported retry/cancellation actions without exposing owner phases as additional choices.
- [x] Preserve the exact retained intent, peer, issuer, and identities across timeout, reload, restart, and authorized retry. Request transient credentials again when the existing contract requires them.
- [x] Closing a page does not cancel accepted work; cancellation acknowledgement is not reported as completed rollback or cleanup. Do not imply automatic resume where explicit authorization is required.
- [x] Proceed to ordinary sign-in only after current readiness and public login evidence verify. Preserve completed source-offline reopen.
- [x] Keep advanced remote connection distinct from Node Join. Selected remote bases must not be replaced with bundled addresses or receive bundled setup capabilities, and connection alone must not provision bundled owners.
- [x] Do not introduce invitation issuance, expiry, redemption, QR infrastructure, or a new pairing protocol. A presentation of existing evidence must preserve peer verification.
- [x] Exercise visible Join behavior at the existing rendered-page seam and reuse public shared HTTPS Join tests for authenticated discovery, approval/rejection, retained retry identity, and offline reopen.
- [x] Delete superseded Join UI and duplicate coordination, update current workflow documentation, and record focused validation outcomes. Approval can use existing source controls until ticket 04 lands.

## Comments

Claimed on setup-simplification-03. Reuse authorized rendered-page and public shared HTTPS seams. Medium presentation complexity, high identity/security risk, high priority; no new worker or pairing protocol.

## Answer

Implemented the dedicated unified `JoinSetup.svelte`: one verified pinned-peer input, default `My Node`, protected authenticated issuer display, source Add a device guidance, progress/blocked states, explicit retained recovery/exact retry/cancellation, transient credentials and current public-login-gated sign-in. Remote targets remain selected and never receive bundled capabilities; target changes refuse actions. Accepted work survives page closure without implicit cancellation, replacement identity or automatic authorized resume.

Deleted the superseded Join form/submission/polling path from Installation setup and exposed the component for ticket 02's parent UI integration. Create, parent default/advanced choices and completion removal, startup and maintenance were not redesigned. Updated `docs/setup-workers.md` and retained the existing shared HTTPS worker/protocol.

Validation: **23 rendered tests**, **181 auth tests**, and clean Svelte check; nine new Join cases use the existing rendered seam and real HTTP adapters. Shared HTTPS lifecycle, feature matrix and CRAP attempts timed out compiling and are **not qualified**. See [full commands, red/green evidence and limits](../evidence/03-streamline-join.md). Integration `23e9006ca` merged/checked (`Already up to date`) before completion; main worktree untouched.
