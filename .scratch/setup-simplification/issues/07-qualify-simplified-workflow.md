# 07: Qualify the simplified workflow and remove remaining obsolete paths

**What to build:** The simplified experience works as one complete flow across Create, Join with source approval, returning-user startup, interruption, advanced remote connections, browser setup, and maintenance. Reviewers receive reproducible evidence and an honestly scoped validation report.

**Blocked by:** 02 — Make Create lead directly to ordinary sign-in; 03 — Make Join a single guided request; 04 — Make adding a Node understandable on the approving device; 05 — Show startup immediately and request trust only when needed; 06 — Separate maintenance from normal onboarding.

**Status:** resolved

**Triage:** ready-for-agent

## Latest user scope decision (2026-10-28)

The milestone requires only the single-device Create/sign-in/startup/recovery experience. Multi-device testing (including a single Join with its source), `cargo hack` feature-powerset testing and CRAP/coverage are not required. Ticket 11 is resolved as dropped by user decision, not passed; ticket 09's parallel/multi-device investigation remains deferred and non-blocking. Existing qualification and implemented Join/security behavior are preserved. This supersedes the broader requirements and ticket11-open references below; no additional multi-device or broad gate runs are needed to keep ticket07 resolved.

## Previous acceptance scope (2026-10-28; superseded where broader)

Per the user's scope decision, qualify **one device undergoing setup at a time**, primarily the bundled desktop. Create/sign-in, startup, cancellation, interruption/exact retry, and retained-identity reopen remain required. Join includes one joining device and its approving source; source-offline reopen remains required after completion. Internal owner coordination and lifecycle contention on that device remain in scope.

Default-concurrency failures from simultaneous independent test scenarios are deferred to [09](09-diagnose-readiness-and-request-timeouts.md), not blockers for this milestone. Sequential execution is the intended qualification mode, not a claim that parallel reliability is fixed. Keep the existing deadlines and all authorization/trust/readiness/durability assertions. Assess the checklist below against current single-device evidence; this decision alone checks no box or resolves this ticket. Explicitly report desktop GUI/provider/trust limits and remaining broad checks under [11](11-complete-provisioned-validation.md).

This section supersedes earlier statements requiring default-concurrency success for ticket 07 closure. Historical outcomes below remain unchanged.

## Current single-device result (2026-10-28)

**Resolved for the current single-device automated workflow scope**, not GUI/platform or release certification. [Single-device evidence](../evidence/07-single-device-qualification.md) records a complete sequential root `owner_boundaries --all-features -- --test-threads=1` pass: **44 passed / 0 failed / 6 existing child-entry ignores**, with the isolated children actually launched by their parents. Both Create/reset variants, both Join/reset variants, and the delayed-authority operation parent passed. A fresh sequential unified HTTPS Join run passed **3/3**, and the setup HTTP client suite passed **24/24**. All supervised workloads exited 0 with no resource stop. No production/test/deadline/assertion edits were needed.

These current passes fill the two integrated single-device gaps below, together with the existing mounted routing/recovery evidence. Historical failures below remain historical, not claims of current parallel reliability. Default-concurrency diagnosis stays open/deferred under 09. [Ticket11 evidence](../evidence/11-single-device-validation.md) records 4/10 root feature combinations completed successfully, the fifth interrupted at the 7200s ceiling, and five unrun; workspace CRAP stopped during instrumented compilation before tests at available RAM below 2 GiB, with no fresh coverage report or CRAP score. Ticket11 remains open/ready-for-human and the spec remains open for broad validation and actual desktop qualification limits. No actual desktop GUI, real provider/OS trust, WAN or packaged release pass is inferred. Parent-authorized reconciliation now updates spec/map alongside this ticket; the earlier spec/map exclusion was delegated write scope, not a user exclusion.

## Previous retry (2026-10-28; historical, before scope decision)

Retry checkpoint on `integration/setup-07-completion`, based on `main` at `31ced611f`; claim released with unresolved failures recorded. The previous single passing operation test did not qualify this ticket. The retry restores a request-local timeout regression without restoring the five-second production budget; fixes cancellation during native HTTPS verification; and releases the lifecycle mutex during registrar work, with post-await exact-operation/cancellation/admission revalidation. It also enforces the existing readiness budget across response-body reads. Concurrent qualification still fails; both durability checklist items remain unchecked. See [retry evidence](../evidence/07-completion-retry.md).

## Current integration status (2026-10-27; historical)

**07, [09](09-diagnose-readiness-and-request-timeouts.md), [11](11-complete-provisioned-validation.md) and the [spec](../spec.md) remain open.** Reviewed changes now include shared-worker/queued-reply and independent connection teardown fences; Storage identity-first denial preserving current policy/response checks; Management policy-schema, standalone HTTP and continuously bound source-HTTPS fixture fixes; and the Device conflict fixture's complete-catalog initial sync (**6/6 focused**, unchanged negative assertions). Application-owned AGENTS guidance (`1cfe76076`) is integrated. Sibling ofdb integration **`a8c23a54540e591aa024396a5674dc016b6cce3a`** includes batched candidate observation, exact field lookup, measured schema stack boundaries and operation-local same-transaction schema reuse with their recorded focused regressions/controls. These are implemented fixes, not whole-workflow acceptance.

The [latest supported root matrix](../evidence/11-validation-attempt.md#supported-qualification-on-current-inputs-2026-10-27) failed combination **1/10 with36p/8f/6ignored**; nine combinations did not run, zero succeeded. Instrumented workspace compilation completed, then `device_contract` failed5p/1f on incomplete catalog import. The [new fixture6/6 pass](../evidence/11-device-contract-fixture.md) is focused only: **matrix/coverage/CRAP have not been rerun**, no fresh report/score exists, and neither supported gate hit a resource/time guard. [Lock-probe evidence](../evidence/09-timeout-localization.md#protected-setup-secret-lock-probe-2026-10-27-utc) records one exact parent passing78.2s with282 acquisition/release pairs and zero contention; it neither identifies the historical initial-driver holder nor implements a behavior fix. Remaining work is causal current-readiness/nested authorization and actual contention-holder diagnosis, followed by unchanged-bound integrated qualification; generated dependency Clippy and GUI/platform/provider/trust/WAN limits remain unwaived. No acceptance boxes change. Older handoff/Answer sections below remain historical receipts.

## Scoped merge handoff (historical)

The user accepted scoped merge readiness with incomplete qualification retained. Final source review of local `main...setup-simplification` at `8d94861d1` found no new code-level blocker; it did not test a merged tree or certify a packaged desktop release. No merge/push was performed as part of this handoff. Ticket 07 stays open and its unchecked acceptance criteria are not silently waived.

Remaining work has concrete follow-ups: [08: provider fixture isolation](08-isolate-mock-provider-fixtures.md), [09: unexplained readiness/request timeouts](09-diagnose-readiness-and-request-timeouts.md), [10: fixture/SDK prerequisites](10-repair-validation-fixture-prerequisites.md), and [11: provisioned matrix/CRAP](11-complete-provisioned-validation.md). Their completion must feed evidence back into this ticket before qualification is resolved.

## Follow-up integration (historical)

The scoped implementation was subsequently merged into local `main`; this follow-up started from newer main `da5bc63419623ab48f93dea14dfce20f3e10ffc9` on `integration/setup-simplification-followup`. The earlier no-merge handoff above is historical.

- [08 provider isolation](../evidence/08-provider-isolation.md) is resolved: deterministic ownership red/green, 16 service unit tests and 35 concurrent owner-boundary tests passed; no claim that every historical signer failure followed the demonstrated sequence.
- [10 fixture/SDK prerequisites](../evidence/10-fixture-prerequisites.md) is resolved: rebuilt exported-SDK scenario, revised Create/Join reset parents and children, neighboring reopen/reset scenarios, six native reset/recovery tests and one peer-revocation test passed.
- [09 diagnostics](../evidence/09-timeout-localization.md) remains open: untraced Join cells passed 12/12, traced approval timed out at 5001ms while the handler later completed at 5439ms; concurrent Create hit a stopped-Device Database lock after awaited shutdown. A later traced pair reopened successfully but failed watchdogs. No deterministic causal regression or lifecycle fix is established; do not mark the two durability criteria qualified.
- [11 resource preflight](../evidence/11-validation-attempt.md) remains open/ready-for-human: SDK prerequisites passed, but approximately 6.8 GiB available RAM and effectively exhausted swap triggered the conservative no-launch gate. Zero matrix combinations and no coverage/CRAP ran in this follow-up. This is not proof that a specific command could not complete.

Independent static review found no further actionable findings after the diagnostic timestamp correction. Focused/scoped passing checks do not establish full workspace, packaged desktop, OS trust, production provider, GUI or WAN acceptance. Ticket07 remains open.

## Qualification checklist

Apply the current single-device acceptance scope above. The [current reconciliation](../evidence/07-single-device-qualification.md#checklist-reconciliation) combines the fresh complete sequential owner suite and unified HTTPS pass with prior mounted/injected evidence. Both integrated durability criteria are now qualified at those seams. Existing checked UI/startup/cleanup criteria retain their prior evidence; they are not new GUI runs. The validation-reporting box records actual attempts and limitations, not completion of the separately tracked broad gates under 11. No fresh broad pass is claimed.

- [x] Verify default bundled first launch has one essential Create/Join choice, minimal required input, no mandatory internal composition, and no completion-only gate before ordinary sign-in.
- [x] Exercise the integrated Join flow from request through source acceptance and exact approval/rejection to verified readiness; verify completed reopen without the source peer.
- [x] Verify accepted work survives disconnect and restart, exact retry retains identities, required transient credentials are requested again, and returning ready Nodes do not replay onboarding.
- [x] Verify visible asynchronous startup, already-trusted launch, consent refusal, actionable prerequisite failure, and lifecycle contention together with the final setup routing.
- [x] Verify selected remote services remain usable without local provisioning or bundled credential forwarding, browser setup never requires Tauri, and independent owners retain separate admission and correlated readiness checks.
- [x] Verify optional local data configuration remains editable without changing restrictions or resource authority, and maintenance reports local deletion separately from incomplete remote revocation.
- [x] Retain negative coverage for authorization, Origin, wrong peer/issuer, secret disclosure, malformed readiness evidence, and lost identity/TLS/provider material. No historical completion marker can manufacture readiness.
- [x] Reuse existing rendered-page and public HTTPS lifecycle scenarios and coordinate with the deployment-independent system-test effort; do not duplicate its scenarios or infer GUI coverage from backend tests.
- [x] Inspect the final call graph and remove any remaining superseded commands, markers, routes, stores, screens, wrappers, or duplicate tests belonging to this workflow. Each preceding ticket must already include its own focused tests and local cleanup.
- [x] Reconcile current glossary-linked design, interface, operations, and native handoff guidance with delivered behavior; clearly distinguish historical evidence from current instructions.
- [x] Record relevant frontend checks/tests, Rust formatting and scoped Clippy outcomes, and actual broad `cargo hack test --feature-powerset --all-targets` / `just crap` attempts and limitations. Fresh HTTP-client/format checks pass; prior scoped Clippy/frontend checks remain historical. Broad matrix/coverage/CRAP completion remains incomplete under 11, not a ticket07 pass or closure prerequisite under the current scope.
- [x] Record which rendered/integration scenarios actually ran and their limits. Injected trust/providers do not certify real OS trust installation, physical/WAN pairing, unsupported platforms, or production deployment. Do not modify real profiles or OS trust stores to satisfy automated validation.

## Comments

- 2026-10-28 final review/parent reconciliation: merged integration tip `4019331e9` before edits. Corrected the prior Answer's attribution: map/spec were excluded only from the earlier delegated write set, not by the user. Parent now authorizes ticket07/spec/map reconciliation. Current scope resolves 07 only; 09 stays open/deferred, 11 stays open/ready-for-human with its bounded evidence, and the spec stays open for broad validation and actual desktop limits. Documentation-only; no tests or heavy commands launched.

- 2026-10-28: Completed single-device qualification on `qualify/setup07-single-device-20261028`. Full root owner suite 44/0/6 existing child ignores, fresh unified HTTPS Join 3/3, HTTP client 24/24, formatting and whitespace checks passed. All launch receipts were polled to actual exit code 0; no resource guards fired. No bug reproduced or code edit needed. Merged current integration tip into the branch before delivery (already up to date at `f2ac31f1d`). Resolution and limitations are in the new evidence and current Answer; no map/spec/09/11 edits.

- 2026-10-28: Claimed on `qualify/setup07-single-device-20261028`, worktree `.worktrees/setup07-single-device-20261028`, based on integration `f2ac31f1da7806929fef1b97c800da894b83c94e` (confirmed merge-base). Qualify the new single-device scope using existing sequential public lifecycle scenarios and unchanged deadlines/assertions; write set is this ticket and new single-device evidence only unless a demonstrated bug is reported first. Parallel failures remain deferred to 09; broad gates remain with 11. No real profile/provider/OS trust changes.

- Claimed on `setup-simplification-07`; qualify the integrated 01–06 implementation at the spec-authorized rendered-page, public HTTPS lifecycle and isolated native provider/trust seams. No real profile or OS trust changes.

## Answer

**Resolved under the new single-device scope.** [Current qualification evidence](../evidence/07-single-device-qualification.md) records exact commands, immutable revisions/lockfile, scheduling, resource receipts, child outcomes and criterion-by-criterion reconciliation. The complete all-features root owner suite passed **44/0/6 existing child-entry ignores** in 1151.89s; fresh unified HTTPS Join passed **3/3** in 180.93s; standalone setup HTTP passed **24/24**; formatting and whitespace checks passed. Existing deadlines, features, security assertions and per-device coordination are unchanged. No demonstrated single-device bug required a production fix.

This closes the two integrated Join/durability evidence gaps for one device undergoing setup at a time, including its approving source. It does not close 09's deferred parallel failures, 11's broad matrix/coverage/CRAP work, the overall spec, or actual packaged desktop GUI/provider/OS trust/WAN qualification. Prior mounted/native/cleanup evidence is reused at its stated seam, not promoted to a new live GUI pass. The initial qualification delivery was limited to this ticket and its new evidence by delegated write scope, not a user exclusion of map/spec. Parent-authorized final reconciliation now updates this ticket, [spec](../spec.md) and [map](../map.md). [Ticket11's later bounded evidence](../evidence/11-single-device-validation.md) records four successful root matrix combinations, a fifth interrupted at 7200s, five unrun, and workspace CRAP stopped below 2 GiB available RAM during compilation before tests; no fresh coverage report or CRAP score. Ticket11 remains open/ready-for-human; these partial results neither reopen 07 nor close the spec.

## Historical answer (before current scope qualification)

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
