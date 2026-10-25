# 07: Qualify the simplified workflow and remove remaining obsolete paths

**What to build:** The simplified experience works as one complete flow across Create, Join with source approval, returning-user startup, interruption, advanced remote connections, browser setup, and maintenance. Reviewers receive reproducible evidence and an honestly scoped validation report.

**Blocked by:** 02 — Make Create lead directly to ordinary sign-in; 03 — Make Join a single guided request; 04 — Make adding a Node understandable on the approving device; 05 — Show startup immediately and request trust only when needed; 06 — Separate maintenance from normal onboarding.

**Status:** open

**Triage:** ready-for-agent

- [x] Verify default bundled first launch has one essential Create/Join choice, minimal required input, no mandatory internal composition, and no completion-only gate before ordinary sign-in.
- [ ] Exercise the integrated Join flow from request through source acceptance and exact approval/rejection to verified readiness; verify completed reopen without the source peer.
- [ ] Verify accepted work survives disconnect and restart, exact retry retains identities, required transient credentials are requested again, and returning ready Nodes do not replay onboarding.
- [x] Verify visible asynchronous startup, already-trusted launch, consent refusal, actionable prerequisite failure, and lifecycle contention together with the final setup routing.
- [x] Verify selected remote services remain usable without local provisioning or bundled credential forwarding, browser setup never requires Tauri, and independent owners retain separate admission and correlated readiness checks.
- [x] Verify optional local data configuration remains editable without changing restrictions or resource authority, and maintenance reports local deletion separately from incomplete remote revocation.
- [x] Retain negative coverage for authorization, Origin, wrong peer/issuer, secret disclosure, malformed readiness evidence, and lost identity/TLS/provider material. No historical completion marker can manufacture readiness.
- [x] Reuse existing rendered-page and public HTTPS lifecycle scenarios and coordinate with the deployment-independent system-test effort; do not duplicate its scenarios or infer GUI coverage from backend tests.
- [x] Inspect the final call graph and remove any remaining superseded commands, markers, routes, stores, screens, wrappers, or duplicate tests belonging to this workflow. Each preceding ticket must already include its own focused tests and local cleanup.
- [x] Reconcile current glossary-linked design, interface, operations, and native handoff guidance with delivered behavior; clearly distinguish historical evidence from current instructions.
- [x] Run relevant frontend checks/tests, Rust formatting and clippy, then final `cargo hack test --feature-powerset --all-targets` and `just crap`. Record actual commands, outcomes, existing unrelated failures, and any unavailable tooling without claiming unrun checks passed.
- [x] Record which rendered/integration scenarios actually ran and their limits. Injected trust/providers do not certify real OS trust installation, physical/WAN pairing, unsupported platforms, or production deployment. Do not modify real profiles or OS trust stores to satisfy automated validation.

## Comments

- Claimed on `setup-simplification-07`; qualify the integrated 01–06 implementation at the spec-authorized rendered-page, public HTTPS lifecycle and isolated native provider/trust seams. No real profile or OS trust changes.

## Answer

**Qualification incomplete; not resolved.** [Current integrated evidence](../evidence/07-integrated-qualification.md) records actual passing, failing and bounded commands and the retained limits. Fixed completed-Create readiness re-observation and removed its premature duplicate sign-in link with two mounted regression cases; fixed the stale native observation test call exposed by compilation. Reconciled current design/API/worker/owner/native/operations guidance and deleted the obsolete route inventory entry.

34 mounted, 180 route/state/client tests, clean app check/build, API 40 plus doc test, native 21 (including provider parent/child), scoped strict Clippy and formatting pass. Workspace Clippy fails generated SDK lints; standalone setup HTTP tests fail. HTTPS Join has individual offline/finalization passes but later approval/Storage/OAuth timeouts; interruption/exact retry is not reliably qualified. Final feature matrix and CRAP each exhausted 600000 ms; no matrix pass/score.

The two unchecked integrated durability/Join items prevent resolution. Resume by reproducing the HTTPS timeouts and completing qualification, not by weakening trust/readiness/authorization assertions or editing real profiles/OS trust. See evidence for unrun process-suite and GUI/platform boundaries. Integration tip was merged before delivery; no main Cargo.lock changes belong to this ticket.


### Review follow-up

[Review-fix evidence](../evidence/07-review-fixes.md): independent ready-page routing, retained-open error kinds, failed recovery retry observation and packaged-native remote setup origins are fixed. Final checks: 193 route/state/client, 34 mounted, API 41 and native 25 pass; clean app check/build, scoped strict Clippy and formatting. Final-code serial public HTTPS finalization/offline/interruption scenarios all passed; a prior repeat failed at real Iroh source-offer polling, so the two integrated reliability boxes remain unchecked. Matrix timed out compiling; CRAP failed on a cached invalid Swagger archive. **Open, not resolved.** No GUI/platform or reliable full-system acceptance inferred.

- Reliability follow-up claimed on `setup-simplification-qualification` at integration `54816fa00`; owns only demonstrated backend HTTPS reliability fixes/tests, ticket07 and new `07-reliability-followup.md`. Existing public HTTPS Join scenarios are the agreed seam; no real profiles/OS trust or sibling changes.

### Reliability follow-up

[New reliability evidence](../evidence/07-reliability-followup.md): source-verified Cargo suite plus six untraced serial repetitions yield **20 passed / 1 failed**. Interruption reopen failed after awaited shutdown with admission `WouldBlock`; separate traced interruption replay yields **1 passed / 1 failed**, reproducing the ordinary authorized source-approval request timeout. No pending-offer timeout recurred here, no common root cause established, no speculative backend patch or relaxed deadline/security assertion. Both durability boxes stay unchecked; **open, not resolved**. Sibling revisions changed independently and are recorded, not modified. No real profiles/OS trust, main source/lock, spec/map or historical receipt changes.

- Focused admission shutdown investigation reclaimed on `setup-simplification-qualification`; source/ownership investigation first while validation compiles. Existing host lifecycle/reopen and admission fail-closed boundaries are the authorized regression seams. No concurrent expensive HTTPS repeats.

### Focused admission lifecycle fix

[Deterministic red/green evidence](../evidence/07-reliability-followup.md#focused-lifecycle-follow-up-deterministic-red--green): proved that the shared worker could report shutdown complete while a connection-owned router retained admission. The worker now joins shared router/control destruction before releasing its host guard; no forced unlock, sleep, retry or timeout change. New lifecycle regression failed before the fix and passes after it (plus 50 serial repeats); setup-host TLS parents/children, retained-fault/recovery checks, two cancellation/retirement tests, scoped strict Clippy/formatting, and one real HTTPS interruption smoke pass. The ownership defect is fixed, not a proven cause of all earlier timeouts. **Open, not resolved**; both durability boxes remain unchecked pending reliable integrated transport/request evidence.

- Reclaimed after merging integration `4ae84f2fc4bbbaf007222859ee48b0e29b641ddf` for the reviewed P1 queued-reply shutdown cycle; deterministic authorized queued-request regression before fix, then focused and serial HTTPS validation.

### P1 queued-reply shutdown review fix

Merged integration `4ae84f2fc4bbbaf007222859ee48b0e29b641ddf` before work. [Latest P1 red/green and final HTTPS evidence](../evidence/07-reliability-followup.md#p1-review-queued-replies-must-be-rejected-before-the-teardown-fence): close/drain/reject buffered command replies before owner cleanup/shared teardown, breaking the cancellation-before-consumption cycle without releasing the host guard early. Public authorized queued-request regression timed out before the six-line fix; after it, queued request rejects, late enqueue denies, both request/worker complete, immediate reopen retains exact admission. Both lifecycle regressions pass 50 serial repetitions each; setup-host suite, cancellation/retirement, retained-state checks, scoped strict Clippy and formatting pass. Final serial HTTPS suites **9/9 scenarios passed** across three repeats. No claim that earlier offer/request timeouts or integrated matrix/coverage failures are eliminated. **Open, not resolved**, durability boxes conservatively retained unchecked.
