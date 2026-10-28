# Ticket 07 — single-device qualification (2026-10-28)

## Disposition

**Ticket 07 is resolved under the explicitly narrowed single-device scope.** The complete root `owner_boundaries` suite passed sequentially, including both independent Create/reset parents, both Join/reset parents, and the operation/reopen parent that failed in the earlier concurrent workload. A fresh sequential unified HTTPS Join suite also passed all three scenarios. No single-device bug reproduced; no production, test, deadline, assertion, manifest or lockfile edit was needed.

This is automated workflow qualification at the existing public lifecycle and injected presentation/platform seams, **not a live desktop GUI or release/platform certification**. Default-concurrency failures remain unresolved/deferred under [09](../issues/09-diagnose-readiness-and-request-timeouts.md). Broad matrix/coverage/CRAP reporting remains incomplete under [11](../issues/11-complete-provisioned-validation.md). Neither those tickets nor the spec/map is changed here.

## Ownership, inputs and execution location

- Own branch: `qualify/setup07-single-device-20261028`.
- Own new worktree: `/home/nathan/code/rust/aicacia/of/.worktrees/setup07-single-device-20261028`.
- Starting HEAD, integration base and confirmed merge-base: `f2ac31f1da7806929fef1b97c800da894b83c94e` (`integration/setup-07-completion`). Claimed ticket07 before qualification.
- Read AGENTS, tracker and triage guidance, current spec/ticket, completion-retry evidence, shared glossary/map, prior integrated/review/HTTP-fixture receipts, and the existing lifecycle tests. Loaded `tdd` and `ponytail`. Reused the user-authorized existing seams; no new test or speculative implementation.
- Complexity/risk/priority: low implementation complexity (documentation-only), high identity/readiness/security qualification risk, high priority. Preserve every existing negative assertion and internal per-device coordination; serialize only independent libtest scenarios.
- Exclusive heavy Rust slot; one Cargo invocation at a time, no overlapping qualification workloads. No unrelated process termination, cache deletion, real profile/provider/OS trust changes, sibling repository writes, or main-branch changes.
- The unchanged resource supervisor hard-codes the integration checkout as its cwd and records that location. Nested-worktree manifests also expect sibling repositories beside the worktree. Instead of changing the supervisor/manifests or adding sibling links outside the write set, execution used the integration checkout with **byte-identical tracked execution inputs**; all authored edits are only in the owned worktree.
- Before execution and again after all runs, compared all **937** tracked files under `crates/`, `tests/`, `src/`, `apps/`, plus root Cargo/build/toolchain/config inputs present in the index: **zero byte differences** between integration and the owned worktree. The sorted manifest digest is `b93a51e1d62c1a64c848066a80dc335bb2e828001aec741dedc95d321259b547` (SHA-256 over each UTF-8 path, NUL, and its binary SHA-256 digest). Integration remained at the stated base with no tracked changes.
- Root/worktree `Cargo.lock` SHA-256: `a39e0f5211fb541b56d9e6cd8c498b2e9ce330c8ca2281c80cd4205ef5b532d1`, unchanged.
- Sibling heads throughout: `ofdb` `6286ca900f7a3fe73fa00e233656afab48d1fb81`; `ofnet` `dbea925a0a3f99053fffc02618baccc2a1cbea9f`; `offs` `df4c696a910296635473ec296d9235acbaa60d9b`. `ofdb` already had untracked `.worktrees/`; other siblings were clean. None was edited.
- Toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)`, Node `v26.5.0`.

## Supervisor and reproduction

Inspected `target/setup07-completion/run.py` before use; reused it unchanged, SHA-256 `723d9bea4a1efdface8ddd9fffe5dcc90022c6bc135b6113b81621807971b32b`.

Each command below was separately launched, then its final JSON receipt polled and checked before launching the next command. A successful launcher is **not** treated as a workload pass. The supervisor sets:

```sh
CARGO_BUILD_JOBS=1
RUST_MIN_STACK=67108864
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target
```

It clears `RUST_LOG`, `RUST_TEST_THREADS`, `TOKIO_WORKER_THREADS`, `OF_SETUP_TIMEOUT_TIMING` and `OF_SETUP_SECRET_LOCK_DIAGNOSTIC`; neither Rust flags variable was set. No diagnostic/timing overrides were enabled. `--test-threads=1` is explicit on the lifecycle suites; Tokio workers, owner concurrency and contention inside each scenario remain unchanged.

From `/home/nathan/code/rust/aicacia/of`, in this order:

```sh
python3 target/setup07-completion/run.py launch single-device-owners-20261028 cargo test -p idp-unified --all-features --test owner_boundaries -- --test-threads=1
python3 target/setup07-completion/run.py launch single-device-unified-join-20261028 cargo test -p unified-server --lib production_api_join_ -- --test-threads=1
python3 target/setup07-completion/run.py launch single-device-frontend-20261028 node --experimental-strip-types --test apps/app/src/lib/common/state/setupApi.test.mjs
python3 target/setup07-completion/run.py launch single-device-fmt-20261028 cargo fmt --all -- --check
```

The supervisor preserves 2 GiB available RAM, 30 GiB available disk, and memory PSI `some avg10 >= 20` or `full avg10 >= 10` sustained for 20 seconds; its overall ceiling is 7200 seconds per command. Termination targets only the launched workload's process group. Every final receipt reports **`exit_code: 0`, `stop: null`**. All four owned workload groups were absent at final inspection.

## Actual current outcomes

| Cell / local receipt label                                     | Actual result                                                                    | Test execution | Supervised elapsed |
| -------------------------------------------------------------- | -------------------------------------------------------------------------------- | -------------- | ------------------ |
| Full root owner suite / `single-device-owners-20261028`        | **44 passed, 0 failed, 6 existing isolated child-entry ignores, 0 filtered out** | 1151.89s       | 1153.635s          |
| Unified HTTPS Join / `single-device-unified-join-20261028`     | **3 passed, 0 failed, 0 ignored, 60 filtered out**                               | 180.93s        | 203.184s           |
| Setup HTTP client contract / `single-device-frontend-20261028` | **24 passed, 0 failed, 0 skipped/cancelled/todo**                                | 288.909447ms   | 1.068s             |
| Formatting / `single-device-fmt-20261028`                      | Passed, empty log                                                                | —              | 3.091s             |
| Owned-worktree `git diff --check`                              | Passed                                                                           | —              | —                  |

The six root ignored entries are existing isolated children, not disabled scenarios. Their parents actually invoke them with `--ignored --exact`; the log records eight successful child invocations (Create twice, Join twice, operation, permissions, remote setup, transfer). Other normal process-child tests also ran. No filters, features or ignores were added to the full root command.

Key independent child results from this **single complete suite**, not pooled focused reruns:

| Parent                                                                       | Child outcome / elapsed |
| ---------------------------------------------------------------------------- | ----------------------- |
| `production_independent_create_and_offline_reopen`                           | 1 passed, 69.19s        |
| `production_independent_create_unsupported_record_refusal_and_current_reset` | 1 passed, 67.67s        |
| `production_independent_join_and_source_offline_reopen`                      | 1 passed, 56.34s        |
| `production_independent_join_unsupported_record_refusal_and_current_reset`   | 1 passed, 58.59s        |
| `production_independent_signer_free_operation_and_delayed_authority_reopen`  | 1 passed, 41.77s        |

Fresh unified parents cover `production_api_join_complete_unretired_finalization`, `production_api_join_completion_and_offline_reopen`, and `production_api_join_interruption_preserves_private_host_intent`. All three isolated HTTPS children passed. The earlier completion retry's 3/3 receipt is corroborating history, not substituted for this fresh run; its sparse source hashes alone did not establish a complete source match.

### Resource observations and raw-log integrity

| Cell                      | Minimum available RAM (KiB) | Minimum free disk (bytes) | Peak aggregate owned RSS (bytes) | Max sampled PSI some/full avg10 |
| ------------------------- | --------------------------: | ------------------------: | -------------------------------: | ------------------------------- |
| Full owner suite          |                     6986248 |              368218144768 |                        376512512 | 0.18 / 0.18                     |
| Unified Join build + test |                     6306640 |              367618195456 |                       4074336256 | 3.42 / 3.35                     |
| HTTP client               |                     7075196 |              367619579904 |                          5767168 | 0 / 0                           |
| Formatting                |                     7037032 |              367619555328 |                         60526592 | 0 / 0                           |

RSS includes the sampled owned Cargo/compiler/test/child processes. In particular the unified peak includes compilation; it is not an application runtime-memory measurement. Short commands can finish between samples.

Disposable `.json`, `.running.json`, `.log`, `.resources.jsonl` and `.supervisor.log` artifacts are retained under `target/setup07-completion/` using the labels above. The `.running.json` files are retained launch records, not indications of remaining processes. Committed evidence records the actual outcomes without depending on those untracked artifacts remaining available.

Raw log SHA-256:

- Owner suite: `4793f1a611f10a3a897466264ed8eb4f10abe4ac86e60a516afbce7a5b879534`.
- Unified Join: `77ea97b902a9357bc2544f4d71d961e82236f4f2f216923e63d7fa1f2836f453`.
- HTTP client: `20795ac3f37c2e048e6941dcef66cf7f7612355309b307f2cf8418d89694f021`.
- Formatting: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

## Checklist reconciliation

| Ticket07 criterion                                                                                           | Evidence and scope                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Minimal Create/Join, default naming, no completion-only sign-in gate                                         | Retain the existing [integrated](07-integrated-qualification.md) / [review](07-review-fixes.md) mounted-page and routing evidence. This task does not rerun mounted tests or infer a visible GUI pass. Current root Create additionally verifies ordinary password sign-in and usable owner routes; fresh unified finalization covers the complete-but-unretired worker checkpoint.                                                                                                                                                                                                                                   |
| Integrated Join acceptance, exact approval/rejection, readiness and source-offline reopen                    | **Now qualified sequentially.** Both independent Join parents pass exact source rejection, unauthorized-approval denial, authorized approval, cancellation, retained restart/retry, current readiness and source-offline use. Fresh unified 3/3 independently covers the bundled public HTTPS path.                                                                                                                                                                                                                                                                                                                   |
| Accepted work, disconnect/restart, exact identities/retry, transient credentials and returning ready routing | **Now qualified at the existing combined seams.** Both Create parents exercise accepted disconnected-caller work, cancellation, restart, conflicting-intent refusal, exact retry, retained Client/endpoint equality, completed reopen and reset safety. Both independent Joins compare exact requests across rejection/restart and exercise source-offline finalization. Unified interruption preserves private host intent. Prior mounted lost-response/recovery cases establish credential re-entry/no frontend persistence and ready routing; those UI cases are retained historical evidence, not newly executed. |
| Visible startup, trust/refusal/prerequisites and contention                                                  | Retain prior mounted/native injected evidence; [completion retry](07-completion-retry.md) adds deterministic cancellation during pending HTTPS verification. No new window-rendering or real trust-installer claim.                                                                                                                                                                                                                                                                                                                                                                                                   |
| Advanced remote/browser and independent admission/readiness                                                  | Prior mounted/browser evidence retained; fresh 24/24 HTTP contract and complete owner suite pass, including remote setup/reopen/reset and separate owner admission.                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Optional local data and maintenance                                                                          | Fresh full suite passes Residency, transfer, permission and remote reset scenarios, plus both independent unsupported-record/current-reset variants. Existing injected maintenance UI/provider refusal/revocation receipts remain relevant.                                                                                                                                                                                                                                                                                                                                                                           |
| Security and fail-closed retained evidence                                                                   | Full owner suite retains current authorization, revocation, denied/late response, Origin, issuer/peer, provider/TLS/identity and secret-redaction assertions; HTTP contract negatives pass. No deadline/security assertion changed.                                                                                                                                                                                                                                                                                                                                                                                   |
| Existing seams, obsolete paths and stable docs                                                               | Prior cleanup/call-graph/domain-doc audits retained. This documentation-only task introduces no replacement path or implementation requiring further cleanup.                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Validation reporting and limits                                                                              | Actual fresh outcomes above; prior scoped API/native/Clippy and rendered results are explicitly historical. Broad gates remain open with 11; no fresh matrix, coverage report, CRAP score, workspace Clippy or full frontend check/build is claimed.                                                                                                                                                                                                                                                                                                                                                                  |

## Remaining limitations, not silently waived

- Sequential success is not proof that historical parallel independent Create/Join/readiness/watchdog failures are fixed. Earlier default-concurrency results and diagnostics remain unchanged under 09. This run found no single-device failure requiring renewed causal diagnosis or a production edit; one complete suite and one fresh unified suite are not an exhaustive flake-rate study.
- `cargo hack test --feature-powerset --all-targets` and `just crap` were **not rerun here**. Their incomplete/failing historical results remain with 11; no matrix-wide, workspace coverage or CRAP completion is asserted. Generated-SDK workspace Clippy issues likewise remain separate; scoped historical Clippy success is not a current workspace pass.
- The passing standalone HTTP fixture suite supersedes its older 9/15 fixture result at that seam only; it is not a mounted UI rerun or frontend-wide acceptance.
- No packaged desktop smoke, actual drawn window, production provider, real OS trust/elevation, physical/WAN pairing, unsupported platform or production deployment was tested. Isolated test roots/providers and injected trust commands are not substitutes for those release checks. The separate opt-in process/system suite was not run; some existing process scenarios do run inside `owner_boundaries`.
- Ticket07 closure is deliberately limited to the user's current automated single-device workflow milestone and honest evidence reconciliation. It neither resolves the overall spec nor certifies a release. Map/spec/09/11 writes were explicitly excluded from this task, so the current disposition is published only in ticket07 and this new evidence.

## Integration handoff

Before delivery, ran `GIT_EDITOR=true git merge --no-edit integration/setup-07-completion` in the owned worktree: **Already up to date**. Integration tip and merge-base remained `f2ac31f1da7806929fef1b97c800da894b83c94e`; all qualified execution inputs therefore remain unchanged. Only ticket07 and this new evidence are delivery paths. No existing worktree was touched, moved or removed.
