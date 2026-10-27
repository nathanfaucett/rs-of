# 11: Complete matrix and CRAP on a sufficiently provisioned runner

**Type:** task

**Status:** open

**Triage:** ready-for-agent

**Priority:** routine qualification

**Blocked by:** 08, 10

## Problem

The feature-matrix attempt reached owner-boundary tests (18 passed / 26 failed / 6 ignored child entry points), then stopped before remaining feature combinations. Coverage compilation was safely stopped when available memory fell to approximately 1 GiB with swap full; no CRAP score exists. The previously truncated Swagger ZIP has been narrowly repaired from a CRC-verified complete local copy.

Read [validation evidence and commands](../evidence/07-validation-followup.md) and [qualification ticket 07](07-qualify-simplified-workflow.md). Human triage is for arranging runner resources and stable inputs, not for changing assertions. Coordinate [09](09-diagnose-readiness-and-request-timeouts.md) if unexplained readiness/request failures recur.

## Acceptance criteria

- [x] Provide adequate measured memory/disk headroom and record immutable source, sibling dependency, toolchain, and lockfile inputs. Do not overwrite user changes to manufacture a clean snapshot.
- [x] Build required workspace SDK artifacts before their test scenarios.
- [ ] Run `cargo hack test --feature-powerset --all-targets` to completion and clearly label root versus workspace scope; do not call a partial first combination a full matrix.
- [ ] Run `just crap` to completion and capture coverage/CRAP results. Preserve build/test semantics rather than changing profiles, suppressing features, or skipping assertions for a green result.
- [x] Record full outcomes and remaining generated-SDK Clippy/standalone setup-test failures separately. Update ticket 07 and the spec only when their acceptance evidence supports it.

## Comments

- 2026-10-27 final supported attempt: [latest evidence](../evidence/11-validation-attempt.md#supported-qualification-on-current-inputs-2026-10-27). First actually merged integration `d35a961fb` into the recreated worktree and verified clean; ofdb fixed at `a8c23a54540e591aa024396a5674dc016b6cce3a`, other siblings/locks stable. SDK setup passed, exports5/5. Exact root matrix exited1 after1137.64s: run1/10 compiled, then owner_boundaries36 passed/8 failed/6 ignored; runs2–10 not launched, zero successful combinations. Actual failures: authenticated Storage and IdP policy-denial timeouts, bounded IdP/Storage readiness, bootstrap secret-operation WouldBlock. No retry/fix or resource stop. Separately `just crap` compiled instrumented workspace in40m54s, passed API42/2ignored, Device server37/1ignored, hosted_node_join2, Device service87, then device_contract5/1 failed at device_contract.rs:225: `Incomplete catalog: data table devices has no definition` during import into empty right fixture, before intended conflict assertions. No suppression/fix; remaining workspace targets unqualified, no coverage report/CRAP score. Both gates stayed above7GiB available; CPU/RSS/PSI receipts captured, no guard/time stop. Eight exact hash/patch-verified own generated paths restored; caches kept, both groups absent. Status open, ready-for-agent for causal coordination; Rust slot released. No map/spec/07 changes.

- Resumed final supported qualification: first fast-forwarded recreated `implement/setup-11-final` from `80da0a99a` to current integration `d35a961fb89df254f20dbf958be37cc6cf5b9784`, verified clean before claiming. Exclusive Rust slot granted. Sibling ofdb now `a8c23a54540e591aa024396a5674dc016b6cce3a`; baseline status will be recorded without altering its existing untracked worktrees. Exact root matrix then separately gated workspace CRAP, shared target/jobs1/stack67108864/default concurrency, 7200s per gate with RAM/disk/sustained-PSI guards and CPU/RSS sampling. No prior failure presumed solved; only11 ticket/evidence delivery scope.

- Final authorized execution at fixed `12c7fda8e`: [full evidence](../evidence/11-validation-attempt.md#authorized-final-execution-2026-10-26). SDK install/build passed and exports5/5. Actual exact root matrix exited1 after1417.03s: first all-features run1/10 compiled, owner_boundaries38 passed/6 failed/6 ignored; nine later combinations not run. Two invalid-bearer Storage response timeouts, one authenticated Storage response timeout, three bounded IdP readiness failures; no resource stop, missing-signer or Database-open failure. No retry/behavior/assertion change. Separately launched actual `just crap`; cold workspace coverage advanced through desktop/root compilation, then safely stopped after3302.64s on three successive full-PSI avg10 samples13.32/17.84/11.43. RAM floor/disk/time limits did not fire; full swap alone did not veto either launch. No coverage report or CRAP score; old LCOV not consumed. Both owned groups absent, caches retained, locks/siblings stable, only eight hash/patch-verified pass-generated tracked paths restored. Historical Clippy/standalone JS failures kept separate, not rerun or waived. Status remains open; human coordination needed for newly failing broader-concurrency ordinary scenarios and pressure-safe coverage continuation. Latest ticket09 scoped serial/threads2 passes remain valid at their declared boundary. No map/spec/07 changes.

- 2026-10-26: Claimed authorized final execution in `.worktrees/setup-11-final`, branch `implement/setup-11-final`, clean base/integration `12c7fda8e97f3ec0e2c8e4000e193122e692b296`. Exclusive Rust slot released by agent09. Exact root matrix then separately gated workspace `just crap`; shared root target, jobs=1, stack=67108864, normal test concurrency. Each gate has a 7200-second overall ceiling with bounded polling, RAM/disk/sustained-PSI guards; full swap alone is no veto. No behavioral/profile/feature/assertion changes or cache deletion. Only this ticket and evidence are delivery scope.

- 2026-10-26: Claimed for the authorized execution attempt on `implement/setup-followup-11`, initially at integration `94c602883`. Tickets 08 and 10 are resolved. Own only this ticket and `../evidence/11-validation-attempt.md`; no map/spec/ticket07 or production changes. Triage marks execution ownership, **not** a provisioned runner. Exclusive Rust slot; jobs=1, stack=67108864, shared target, bounded commands and owned-process-group resource guards. No unrelated process termination, cache deletion, profile/assertion/semantic changes or retry-to-green.

- Execution disposition: [ticket11 evidence](../evidence/11-validation-attempt.md). Frozen JS install and four owner-client ESM builds passed; built exports **5/5 passed**. Rust preflight measured 6.82 GiB available RAM, 136 KiB free swap, 195.73 GiB disk. Exhausted swap failed the safety gate, so **Cargo matrix was not launched (zero combinations)** and `just crap` was not run. No compiler/test failure, matrix completion or CRAP score claimed. Ticket returns to **open**, not resolved; runner provisioning remains unmet. The eight pass-generated tracked artifacts were subsequently verified against the saved pass patch and restored to the clean pre-pass baseline at the user's request; only ticket/evidence are commit scope. Owned JS process groups are absent; no Rust process was created. No retry-to-green or ticket09 diagnosis/fix.

- Cleanup follow-up: status remains **open**, triage **ready-for-human** because arranging a sufficiently provisioned runner remains an external blocker. Swap exhaustion prompted a conservative **no-launch safety decision**, not demonstrated inability of any specific command to execute or complete. Neither matrix nor CRAP executed in ticket11. All eight generated changes matched the saved pass patch exactly; no unknown user changes were found or restored. No tests/builds launched during cleanup.

This does not authorize real profiles, OS trust installation, or production provider changes. Packaged desktop smoke testing with a disposable profile remains a release gate outside injected/backend qualification; full GUI/platform/WAN certification is not inferred from these commands.
