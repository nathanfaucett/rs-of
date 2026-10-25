# Setup simplification map

## Notes

- Delivery follows [the spec](spec.md) and numbered tickets. Each worktree owns its assigned slice; source approval UI is ticket 04.
- Ticket 01 started on `setup-simplification-01` at integration `setup-simplification` / `dfeaf7dbe`; the integration merge was already up to date.

## Decisions-so-far

- [01: Unify onboarding observation and recovery routing](issues/01-unify-onboarding-observation.md) — resolved. All onboarding/authentication routing consumes the target-bound typed observation. Shared worker evidence remains authoritative, remote/browser observation stays direct HTTP, native capability retrieval is separate and admission parsing is shared. Superseded observation commands/functions are deleted. [Validation evidence](evidence/01-onboarding-observation.md).
- Preserve the Client-bound Device preference step in this observation-only slice. Later tickets may remove its dismissal gate without weakening current readiness or changing actual local selection permissions.
- Safe failure actions distinguish transient retry, prerequisites and exact retained-identity repair; destructive reset remains explicit maintenance, not automatic recovery.
- [04: Source approval](issues/04-simplify-source-approval.md) resolved: one Add a device task, existing owner authorization/adapters, exact decisions and retained partial-approval retry. [Focused evidence and validation limits](evidence/04-source-approval.md): 9 rendered/27 owner-client tests pass; Svelte check/autofix clean. Rust lifecycle, feature matrix and CRAP bounded out compiling; broad auth suite is not green.

- [02: Streamline Create](issues/02-streamline-create.md) — resolved. Credentials-only bundled Create, dependency-free default naming with ordinary rename, exact transient-credential recovery, direct verified ordinary sign-in and deferred authenticated Device preferences. Removed dismissal route/command/flag/persistence; actual restrictions, selections and resource authorization are unchanged. [TDD, caller audit and validation evidence](evidence/02-streamline-create.md). This supersedes ticket01’s temporary preservation of the dismissal gate.
- [03: Guided Join](issues/03-streamline-join.md) — resolved. Dedicated Join UI defaults the name, displays protected authenticated Installation evidence, preserves exact recovery/retry/cancellation and selected remote targets, and gates sign-in on current public login. Parent changes only wire/delete Join; 02/05/06 ownership remains intact. [Evidence](evidence/03-streamline-join.md): 23 rendered/181 auth tests and Svelte check pass; shared HTTPS/matrix/CRAP timed out compiling.
- [05: Visible startup and trust](issues/05-visible-startup-and-trust.md) — resolved. Window-first asynchronous retained-host startup, current native-root verification before conditional explained consent, refusal/retry, cancellation/join and late-publication fencing. No Create/Join/completion or maintenance presentation rewrite. [Focused evidence and limits](evidence/05-visible-startup-and-trust.md): 22 native, 15 mounted and 181 route/state tests pass; existing provider readiness bound reproduced with original startup, full matrix/CRAP compilation timed out. No real OS trust/profile/GUI qualification.
- [06: Separate maintenance](issues/06-separate-maintenance.md) — resolved. Dedicated bundled-only `/maintenance`, per-attempt destructive confirmation, distinct advanced exact-reference recovery and explicit-bearer peer revocation. Native reset/runtime internals preserved. [Evidence](evidence/06-maintenance.md): 21 mounted interactions, 28 setup-page cases, build/check pass; broad auth and native/feature/CRAP qualification remain limited.

- [07: Integrated qualification](issues/07-qualify-simplified-workflow.md) — **open, not resolved**. Fixed Create completed-recovery/current-login routing, deleted the premature duplicate sign-in path and repaired a stale native test signature. 34 mounted/180 route-state-client, API 40, native 21 and scoped strict Clippy pass. Mixed HTTPS lifecycle timeouts and incomplete final matrix/CRAP prevent acceptance. [Current evidence and limits](evidence/07-integrated-qualification.md).

## Fog

- Current ticket07 HTTPS Join runs are inconsistent: individual offline/finalization passes coexist with OAuth, approval and Storage request timeouts; interruption/exact retry remains unqualified. No baseline cause asserted.
- Final Cargo Hack matrix and CRAP each exhausted 600000 ms compiling; no complete matrix or score. Earlier Swagger archive failure is historical, not the current observed blocker.
- Workspace strict Clippy fails 93 generated IdP SDK lints; scoped API/native strict Clippy passes. Standalone setup HTTP fixtures still fail; the 180-test Vitest route/state/client suite and 34 mounted interactions pass.
- Existing deployment-independent process scenarios were inspected, not rerun; referenced scratch reports are absent. Do not infer their results or duplicate their suite.
- Real OS trust/elevation/provider profiles, full desktop GUI, physical/WAN and other platforms remain outside isolated same-host injected evidence.


## Review follow-up

Single-implementer `setup-simplification-review` fixes independent ready-page sign-in, preserves operational retained-open errors, publishes stopped/retry after reference recovery, and supports exact configured packaged-native origins for remote HTTPS services without bundled capabilities. [Evidence](evidence/07-review-fixes.md): final frontend 193 + mounted 34, API 41, native 25, scoped strict Clippy/check/build pass. Final-code serial HTTPS Join finalization/offline/interruption **3 passed**, but a preceding repeat had a source-offer timeout; reliability remains open. Latest matrix still timed out; latest CRAP failed on a malformed cached Swagger archive rather than timing out. Earlier Fog/receipts retain their historical outcomes. Ticket07/spec remain **open**.
