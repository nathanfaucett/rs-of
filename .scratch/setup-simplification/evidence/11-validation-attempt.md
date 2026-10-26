# Ticket 11: validation execution attempt

Latest result: [authorized final execution](#authorized-final-execution-2026-10-26) actually launched both gates: root matrix failed 38/6/6 in run1/10; workspace coverage stopped on sustained PSI after55 minutes, without a score. Ticket11 remains open. The original no-launch attempt below is historical.

## Original attempt disposition

**Open; safety-preflight blocked. No Rust matrix process was launched, no feature combination completed, and no coverage/CRAP result exists from this attempt.** This is not a partial first-combination result, compiler/test failure, timeout, or provisioned-runner qualification. JS prerequisites did execute successfully. Swap exhaustion prompted a conservative no-launch safety decision; this does **not demonstrate that any specific command could not execute or complete**. Neither matrix nor CRAP executed in this ticket11 attempt. The safety condition was not relaxed merely because available RAM exceeded 2 GiB.

## Ownership and inputs

- Date: 2026-10-26. Worktree `/home/nathan/code/rust/aicacia/of/.worktrees/setup-11`, branch `implement/setup-followup-11`. Initial clean HEAD and integration tip: `94c60288315075b0b34861a98916e0721e56ceb5`. Claimed ticket before execution; triage `ready-for-agent` denotes this execution attempt, **not runner provisioning**. Tickets 08 and 10 are resolved; 09 remains unresolved.
- Read worktree AGENTS, issue-tracker/triage instructions, spec, ticket11, ticket07, evidence08/10/09 (including latest clock correction and native-lock diagnostic), and ticket07 validation diagnosis. Loaded ponytail. No implementation, dependency, profile, timeout, feature, test assertion, provider, OS trust or real-profile changes.
- Exclusive Rust slot was granted, but no Rust build was ultimately started. Only this receipt and ticket11 are commit scope; map/spec/07 and sibling sources are untouched.
- Complexity/risk/priority: no implementation complexity; material host resource/exhaustion risk; routine qualification. Reuse existing frozen install, SDK build and export checks, with no new repository harness or production edits.

Observed immutable Git inputs (archive hashes describe committed HEAD, not generated artifacts):

| Repository | Full HEAD                                  | SHA-256 of `git archive HEAD`                                      |
| ---------- | ------------------------------------------ | ------------------------------------------------------------------ |
| of         | `94c60288315075b0b34861a98916e0721e56ceb5` | `7b86ab28af3414284936b73c8cce3a1b571d56ada8bda424de4b155e4a276ac0` |
| ofdb       | `e3242170c94d5fc4bcc121db812310c8106c7ab6` | `b9c32a875b4c67552ecd9e30461cb381be1418e322664fe8bcb6a402724d3a43` |
| ofnet      | `dbea925a0a3f99053fffc02618baccc2a1cbea9f` | `bfba0566e795912b15cb555a3f9f724244c950bee11f18a5ea9ca73f031fc582` |
| offs       | `df4c696a910296635473ec296d9235acbaa60d9b` | `7fe57bb321e805a52c991a299e83aa38e23ac77a637be8d83a100c937a2c97f1` |

Siblings were clean at input capture; existing `.worktrees/{ofdb,ofnet,offs}` symlinks resolve to these repositories. No symlink changes. These are observed input receipts, not a hermetic snapshot or proof no independent process could change inputs.

| Input                          | SHA-256                                                            |
| ------------------------------ | ------------------------------------------------------------------ |
| Worktree and main `Cargo.lock` | `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f` |
| `pnpm-lock.yaml`               | `dd1bdc36e5c990c1855ff53bec8d62c472c6090d23f62cbe5397ec8554cd4fa5` |
| `rust-toolchain.toml`          | `a6a0bbd29ffaa8182dc22d1d9149709f1091e47df40ed96eb8a78a711c66a4ce` |
| `.cargo/config.toml`           | `765dd4126c2fb7df33ba9ab4a0dc1664d91fcab0789013b31b55306c5d3f345a` |
| `Cargo.toml`                   | `f17989274453ceaf571542e9207e58cb4442fd287df6a443e630748ed09191db` |

Actual tools: rustc `1.99.1 (58cf8f9c3 2026-10-13)`, full compiler commit `58cf8f9c35b355199727627d07ae34a4e2846d67`, host `x86_64-unknown-linux-gnu`, LLVM `23.1.1`; cargo `1.99.1 (5f94df478 2026-08-27)`; cargo-hack `0.6.44`; cargo-llvm-cov `0.8.7`; cargo-crap `0.5.0`; Node `v26.5.0`; pnpm `12.4.1`; just `1.49.0`. Declared toolchain is stable with rustfmt/clippy. No inherited CARGO/RUST/SWAGGER environment overrides were present.

## Resource baseline and safety decision

Initial measurement: RAM total **32,967,720,960 bytes (30.70 GiB)**, available **7,432,896,512 bytes (6.92 GiB)**; swap total **2,147,479,552 bytes**, free **77,824 bytes (76 KiB)**; disk available **210,193,256,448 bytes (195.76 GiB)**. Later input capture at 14:21:09 UTC measured available RAM 7,448,788,992 bytes and swap free 86,016 bytes. Earlier reported 17 GiB headroom was not present in these actual measurements.

Largest initial RSS consumers (KiB, process names only): editor PID2785858 5,427,248; rust-analyzer PID2965212 5,373,072; Node PID2788334 3,043,136; rust-analyzer PID3062670 2,460,264; Node PID2788349 2,050,836; llama-server PID415287 1,490,408. These are independent workloads, not owned validation processes. No process was killed to manufacture headroom. Memory PSI avg10 some/full was 0.00 at input capture and Rust preflight; full swap alone is not evidence of active thrashing or a cause of ticket09's failures.

After JS prerequisites, the Python Rust supervisor measured:

- MemAvailable **7,326,633,984 bytes (6.82 GiB)**.
- SwapFree **139,264 bytes (136 KiB)** out of 2,147,479,552 bytes.
- Disk free **210,167,742,464 bytes (195.73 GiB)**.
- PSI avg10 some/full **0.00/0.00**.

Guard policy: reject launch or terminate the owned process group on available RAM `<2 GiB`, operationally exhausted swap (`SwapFree <1 MiB` when swap exists), or disk `<30 GiB`; the swap cutoff is an explicit conservative interpretation of exhausted swap, not a claim that the kernel reported literally zero free bytes. Tens/hundreds of KiB is not meaningful swap headroom. **The swap condition was already true at preflight**, so the supervisor did not call Popen for Cargo. Do not present its successful Python exit as a successful Cargo execution.

The prepared parent would start Cargo with `start_new_session=True`, sample every second, enforce a 555-second outer bound, and TERM only its owned PGID, wait up to 10 seconds, then KILL/reap and clear remaining group members. Its tool invocation was bounded to **600000 ms**. No cleanup signal or elapsed/resource-stop event occurred for Rust because no Rust child existed. No bounded compilation continuation was warranted: there was no started compilation/progress and safety had failed.

`just crap` was **not started**: the same measured exhausted-swap condition made instrumented workspace compilation unsafe under the requested safety gate. Historical coverage also exhausted memory, but that history is not claimed as a new measured coverage result. No coverage cache deletion/repair, new profile, linker flag, instrumented-target override or score fabrication.

## Commands and actual outcomes

All commands used this worktree as cwd. No command exceeded 600000 ms.

| Command                                                  | Outcome                                                                                                                                                                                  |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm install --frozen-lockfile --ignore-scripts`        | Exit 0; 13 projects, 465 packages reused, no downloads; supervised 1.001s. Lockfile unchanged.                                                                                           |
| `pnpm sdk:build`                                         | Exit 0; existing IdP, Management, Device and Storage `build:esm` scripts succeeded; supervised 4.003s.                                                                                   |
| `node --test scripts/generated-package-exports.test.mjs` | **5 passed**, 0 failed/skipped; 139.68ms reported test duration. This verifies built exports, not the Rust backend/router scenario.                                                      |
| `cargo hack test --feature-powerset --all-targets`       | **NOT LAUNCHED: exhausted-swap preflight.** Exact prepared argv; root `idp-unified` scope, no `--workspace`. Zero combinations executed. No compiler, test failure, or timeout reported. |
| `just crap`                                              | **NOT RUN: unsafe resource baseline.** Recipe would build SDKs then run workspace llvm-cov followed by cargo-crap only on success. No coverage or score.                                 |

Prepared Rust environment, unchanged semantics:

```text
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target
CARGO_BUILD_JOBS=1
RUST_MIN_STACK=67108864
```

No matrix repeat, test filtering, test-thread override, assertion relaxation or diagnostic repair was attempted. No cause of ticket09's approval/readiness timings or reproduced stopped-Device-Database lock failure was inferred or fixed. Its latest tracing run had successful post-shutdown opens followed by watchdog failures, not a proven lock fix. Keep those families separate from this host safety blocker.

Previous generated-SDK workspace strict Clippy and standalone setup-contract failures remain **historical, not rerun**: ticket07 integrated evidence records 93 generated `of-idp-client` lint errors and standalone `setupApi.test.mjs` 9 passed/15 failed. Ticket09 records additional scoped test-target lints. This receipt neither waives those failures nor claims their current reproduction. No full OpenAPI generation, backend SDK scenario, GUI, physical/WAN, real provider/trust or packaged desktop qualification.

## Generated output preservation and process cleanup

Initial worktree was clean. Frozen installation/build caused these **eight tracked changes**, deliberately retained in place, not restored or staged into the two-file evidence/status commit:

- `packages/storage-client/esm/index.js.map` (mapping offset regenerated by existing TypeScript compiler).
- `packages/storage-client/node_modules/.bin/{biome,rolldown,shx,tsc,tsserver,vitest}` (generated launcher paths).
- `packages/storage-client/node_modules/vitest` (generated peer-dependency symlink).

Ignored installed dependencies and built ESM artifacts also remain available; no generated outputs or caches were deleted. A private binary patch preserves all eight tracked generated changes at `/tmp/setup11-generated.patch`, SHA-256 `46a3a6a704be321dd6e2a8c93ad4139c7cc9cc12126d268bebdb90b64bf92baa`. Those generated changes were initially left dirty intentionally; the authorized cleanup below supersedes that disposition. Both Cargo locks and pnpm lock retained the hashes above after JS builds.

Python parent supervision used separate sessions for install PGID **3078622** and SDK PGID **3078764**, with RAM/disk/540s safeguards, finally TERM/KILL of only those owned groups and wait/reap. Both children exited 0; a subsequent `killpg(pgid, 0)` found **both groups absent**. The lightweight prerequisites ran before the exhausted-swap heavy-Rust preflight; do not claim a passed Rust safety gate for them. No Cargo/rustc/test/just process was created by this attempt, and no unrelated editor/system process was terminated. Exclusive Rust slot is released with this report.

## Private receipts and remaining gate

Mode-0600 local captures (disposable, not repository harness files):

| Path                       | SHA-256                                                            |
| -------------------------- | ------------------------------------------------------------------ |
| `/tmp/setup11-inputs.json` | `50e9681050751c2f85e9fb8b8278ec1d074e430ea1add39d70fbc8038f76964a` |
| `/tmp/setup11-install.log` | `661de1635e6d29a9b42f5672c2fd6d98dfe5ec0673a1ca9384f8970e0c3e4bb9` |
| `/tmp/setup11-sdk.log`     | `904f7e226e40f0814fee6bac65d5823748e61050a713d72845384298d4a58f40` |
| `/tmp/setup11-js.json`     | `87b59b0ead4bd2230398dfdfea26ad9c9e89550146007d70cbf0f20726dd5870` |
| `/tmp/setup11-matrix.json` | `00a1dc9f9991201a8e52e33bff7fb22ca2009b4a0b2e20503380e77064224821` |

`/tmp/setup11-manifest.json` lists those hashes and the generated patch. Matrix JSON explicitly records `launched: false`; no Cargo transcript exists because Cargo never ran.

Final checks: `git diff --check` passed; all three siblings remained clean at the recorded HEADs; both Cargo locks and pnpm lock still matched. After the evidence/status commit, `GIT_EDITOR=true git merge --no-edit integration/setup-simplification-followup` returned **Already up to date** against integration `94c60288315075b0b34861a98916e0721e56ceb5`. No integration branch movement was performed. Only the eight documented generated paths remained dirty.

## Authorized tracked-artifact cleanup

At the integration coordinator's subsequent request, restored only the eight tracked generated paths listed above to pre-pass commit `94c60288315075b0b34861a98916e0721e56ceb5`. Before restoration:

- Verified `/tmp/setup11-inputs.json` against its recorded SHA-256; its pre-build status contained only the agent's ticket claim, with no generated/user edits. The initial entry status had been clean.
- Verified `/tmp/setup11-generated.patch` against its recorded SHA-256, and required the current entire binary diff under `packages/storage-client` to match it byte-for-byte, with exactly those eight changed paths.
- Verified each path's committed HEAD bytes matched the pre-pass baseline and that the index was empty. No unknown user changes were found; none were restored.

Used `git restore --source=94c60288315075b0b34861a98916e0721e56ceb5 --worktree -- <eight explicit paths>`. Immediately afterward `git status --porcelain=v1` returned empty. The private generated patch remains available; ignored dependencies/build outputs and caches were not deleted. Cleanup launched **no tests or builds**. Only ticket11/evidence documentation was then changed for the cleanup commit. Triage is now **ready-for-human**, status **open**, because runner provisioning remains an external blocker. The exhausted-swap preflight was a no-launch decision, not an experimentally demonstrated failure or inability of the matrix/CRAP commands.

**Ticket11 remains open.** Resume only with measured safe resource headroom and stable inputs, using the exact root matrix and then a separately gated `just crap`; record full versus partial execution honestly. No runner was provisioned here. No changes to ticket07/spec/map are authorized or justified by these results.

## Authorized final execution, 2026-10-26

This section supersedes the preceding no-launch disposition for the new attempt only. **Both actual gates launched. Root matrix failed in its first combination; workspace coverage stopped on sustained memory PSI. No complete matrix, coverage report or CRAP score. Ticket11 remains open.**

### Ownership and fixed inputs

- Worktree `/home/nathan/code/rust/aicacia/of/.worktrees/setup-11-final`, branch `implement/setup-11-final`. Entry HEAD, integration tip and merge-base all `12c7fda8e97f3ec0e2c8e4000e193122e692b296`; entry status clean. Claimed11 with triage `ready-for-agent` before execution. Agent09 released the exclusive Rust slot. Loaded ponytail and TDD; this is qualification of existing seams, not a new behavior/test implementation.
- Low implementation complexity; material host resource and lifecycle qualification risk; routine qualification priority. No source, behavior, dependency, feature, profile, linker, assertion, timeout, provider, trust or real-profile changes. Only ticket11 and this evidence are delivery scope; map/spec/ticket07 untouched.
- Root Cargo.toml, Cargo.lock, pnpm-lock.yaml, toolchain and Cargo config match the earlier receipt's hashes. Rechecked unchanged after prerequisites, between matrix/coverage and after both gates. Justfile SHA-256 `ce1ff6ab7981d8e26883924a9f890ce78a5ce15ea1ca880c91dd31372d0f828a`. Cargo.lock SHA-256 remains `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`.
- Siblings clean throughout the recorded checks at ofdb `e3242170c94d5fc4bcc121db812310c8106c7ab6`, ofnet `dbea925a0a3f99053fffc02618baccc2a1cbea9f`, offs `df4c696a910296635473ec296d9235acbaa60d9b`. Existing `.worktrees/{ofdb,ofnet,offs}` symlinks reused unchanged. No source input drift found.
- Full tool versions/environment recorded in `/tmp/setup11-final-inputs.json`: rustc 1.99.1 / LLVM 23.1.1, Cargo 1.99.1, cargo-hack 0.6.44, cargo-llvm-cov 0.8.7, cargo-crap 0.5.0, Node v26.5.0, pnpm 12.4.1, just 1.49.0. No inherited CARGO/RUST/SWAGGER overrides.
- `/tmp/setup11-final-source-receipt.json` records full HEADs/tree hashes/archive hashes. Root `git archive HEAD` SHA-256 `79db8e7d92795cb7b93174f5f64545cb079407a3e00c7be3825c1354425f4999`; sibling archive hashes match the previous immutable-input table. These are observed fixed inputs, not filesystem isolation from independent writers.

### Prerequisites and execution protocol

`pnpm install --frozen-lockfile --ignore-scripts && pnpm sdk:build && node --test scripts/generated-package-exports.test.mjs` **passed**, exit0, 4.48s supervised. All 465 packages reused, zero downloads; all four owner-client ESM builds passed; built exports **5/5 passed**. Worktree-local dependencies installed using the existing entry point; no root node_modules replacement/symlink or tracked artifact substitution. `just crap` subsequently rebuilt the four SDKs successfully through its normal prerequisite.

Actual Rust environment, unchanged for both gates:

```text
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target
CARGO_BUILD_JOBS=1
RUST_MIN_STACK=67108864
```

Normal test concurrency retained: no RUST_TEST_THREADS, libtest filter/thread override, feature exclusion or profile/optimization changes. Root powerset is **not workspace scope**: cargo-hack advertised **10 runs**, first `cargo test --all-targets --all-features` on `idp-unified`. Workspace coverage covers 23 workspace packages with default-feature resolution/unification, not a powerset or all-features/all-targets matrix. llvm-cov explicitly logged enabling **cfg(coverage)**; no coverage-named Cargo feature was added. Existing opt-in ignored system scenarios are not certified.

Root target had been independently recreated before this attempt; recent normal focused artifacts reused where Cargo allowed, not the historical full cache. Coverage required a cold distinct `target/llvm-cov-target`. No caches deleted, archive repair applied, warmup build or instrumented-target override.

Private disposable supervisor `/tmp/setup11-final-run.py` launches each command in a fresh owned PGID and records five-second MemAvailable, SwapFree, disk, PSI avg10 and aggregate owned RSS samples. Every launch/poll tool invocation was bounded below 600000ms; **the child continued across polling calls without stage restarts**. Each Rust gate had a 7200-second overall ceiling. Stops: MemAvailable <2 GiB; disk <30 GiB; some avg10 >=20 or full avg10 >=10 for three consecutive samples; overall time ceiling. TERM then KILL if required applies only to owned PGIDs, with child wait/reap and residual-group cleanup. No unrelated process terminated. Full swap alone was not a launch veto.

Initial preflight: approximately **5.8 GiB MemAvailable**, essentially full 2 GiB swap, **390 GiB disk free**, some/full PSI avg10 **0/0**. Inspected cgroup ancestors had unlimited memory.max/memory.high. These measurements permitted a guarded attempt, not an assurance of completion.

### Exact root matrix: actual test failure, no retry

Command: `cargo hack test --feature-powerset --all-targets`, cwd this worktree, PGID **3276937**.

- Cargo-hack **exit1**, inner Cargo **exit101**, supervisor wall **1417.03s**. No guard or overall timeout fired.
- First all-features run **1/10** compiled successfully in **18m56s**. Root main unit target: zero tests, successful.
- `owner_boundaries`: **38 passed / 6 failed / 6 ignored**, zero filtered; **280.44s**. Existing child-only ignores retained. **First combination failed; remaining nine combinations/later targets did not execute. Zero fully successful matrix combinations; no full matrix qualification.**
- No retry, test-thread reduction, assertion relaxation or behavioral fix after the failure.

All six failed parent names under `owner_composition_tests::`:

| Parent                                                                                  | Actual child failure                                                                           |
| --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `create::production_independent_create_and_offline_reopen`                              | `create.rs:772`: invalid Storage bearer request timed out before receiving expected HTTP401.   |
| `create::production_independent_create_unsupported_record_refusal_and_current_reset`    | Same `create.rs:772` HTTPS Storage timeout; not a retained-reset assertion failure.            |
| `operation::production_independent_signer_free_operation_and_delayed_authority_reopen`  | `create.rs:443`: authenticated Storage `/databases` request timed out before expected HTTP200. |
| `join::production_independent_join_and_source_offline_reopen`                           | `create.rs:250`: `bounded independent idp readiness`.                                          |
| `join::production_independent_join_unsupported_record_refusal_and_current_reset`        | Same bounded IdP readiness failure, before qualification of the reset assertions.              |
| `remote_setup::remote_create_join_reopen_and_device_reset_preserve_shared_installation` | Same bounded IdP readiness failure.                                                            |

Parent wrapper panics are at create.rs:108, operation.rs:82, join.rs:114 and remote_setup.rs:80; they are consequences of the above child failures, not additional independently diagnosed causes.

**Diagnosis boundary:** actual ordinary HTTPS response/readiness failure families associated with ticket09, newly reproduced under broader root all-features/default concurrency. No missing-private-signer or `Database already open` text occurs in this log. SDK prerequisites passed and no fixture schema/reset rejection is the observed failure. No resource guard fired; runtime samples commonly had PSI0 and about5 GiB available. This does **not** prove host load/concurrency irrelevant, nor establish the production handler/owner cause. No speculative fixture or behavior repair is justified from these lines alone.

The latest ticket09 evidence resolved its explicitly declared untraced serial and threads=2 cells at fixed no-Swagger feature choices (16/16 parents and current causal regressions passed). Preserve those scoped passes: this all-features, full default-concurrency failure is different qualification evidence, not a retroactive failure of those cells. Coordinate the new actual failures before changing behavior; no failed scenario was rerun here.

### Exact just crap: sustained-PSI stop, no score

Command: `just crap`, cwd this worktree, PGID **3292912**. Launched only after matrix group absence and input/resource recheck.

- SDK prerequisite passed; llvm-cov actually launched and cold instrumented compilation advanced through GTK/WebKit/Tauri, desktop `lidp`, plugin and root packages. No compiler error was observed before the stop; existing unused test-support warnings remained.
- Supervisor wall **3302.64s** (55m02.64s), command exit **143**. **Stop reason: sustained PSI**, not RAM floor, disk, full swap or overall/stage timeout.
- Three consecutive five-second samples at elapsed **3290.57 / 3295.60 / 3300.61s** had full avg10 **13.32 / 17.84 / 11.43**, above10 each; some avg10 **14.45 / 19.08 / 12.11**. Available RAM at these samples was **4,673,712,128 / 5,576,843,264 / 5,394,501,632 bytes**. Sampling satisfies the declared three-sample guard; it does not identify which independent host workload caused the stalls.
- Compilation/linking was still in progress. No `Finished test`/test suite completion or fresh coverage report. **cargo-crap was not reached**, because the recipe chains it with `&&`. No score or coverage percentage exists from this attempt. An older `/tmp/lcov.info` remained in place, size135599, mtime1792952336.3300025; it is not this attempt's report and was not consumed for a fabricated score.
- No restart under pressure. Completed normal/instrumented artifacts preserved for coordinated continuation; no cache cleanup or repair. Both Rust owned groups verified absent after execution; no build/test/coverage child from this attempt remains.

### Sampled resources

RSS is summed per owned PGID, not a physical-memory or kernel high-water guarantee; between-sample peaks are unknown.

| Gate                      | Minimum MemAvailable bytes | Minimum disk-free bytes | Maximum sampled owned RSS bytes | Max PSI some/full avg10 |
| ------------------------- | -------------------------: | ----------------------: | ------------------------------: | ----------------------- |
| Root matrix               |              4,207,054,848 |         405,081,718,784 |                   4,852,727,808 | 7.26 / 6.36             |
| Workspace coverage recipe |              4,168,691,712 |         374,012,731,392 |                   5,303,189,504 | 19.08 / 17.84           |

### Artifact preservation, remaining checks and handoff

Installation/build generated exactly the same eight tracked path categories as the previous attempt: Storage ESM index map; node_modules `.bin/{biome,rolldown,shx,tsc,tsserver,vitest}`; node_modules/vitest symlink. Initial clean baseline and pre-build claim-only status recorded. Saved exact post-build path/symlink/content hashes and binary patch. After both gates, required entire packages binary diff to equal that saved patch, exact path set equality, per-path hashes and unchanged base HEAD/empty index before `git restore --source=12c7fda8e97f3ec0e2c8e4000e193122e692b296 --worktree -- <eight explicit paths>`. No unknown/user changes restored. Only ticket/evidence remained dirty; dependencies and compiled caches preserved.

Private mode0600 receipt set `/tmp/setup11-final-manifest.json` includes hashes/sizes of supervisor, immutable inputs, source receipt, generated patch/hash receipt, logs, result JSON and complete resource JSONL. Important log SHA-256:

| Log                             | SHA-256                                                            |
| ------------------------------- | ------------------------------------------------------------------ |
| `/tmp/setup11-final-prereq.log` | `bcba52355a4970ce1103655cf2649b172f92f0707a6dad60260a6f5c09bca0c0` |
| `/tmp/setup11-final-matrix.log` | `74a753c9e73d098347cf68d8842822b70b8bb3579aeb7258563e3c7b71170c78` |
| `/tmp/setup11-final-crap.log`   | `8b3bbc79625ce9083ba1204dc356c0e22c1213c2ce2bab3e824f90196bcaa83b` |

Historical generated `of-idp-client` strict Clippy93 errors and standalone setupApi9-pass/15-fail receipt remain **not rerun here**, not waived or claimed current. The separate JS-fixture agent owns any new standalone qualification; no result from its separate worktree is incorporated without evidence. API-only historical Clippy success does not certify generated workspace/test targets. No full workspace Clippy launched after the sustained-pressure stop. No GUI/WAN/platform/provider certification.

Ticket11 remains **open**, triage **ready-for-human** for coordination of the six concrete ordinary failures and a pressure-safe coverage continuation/runner. The previously speculative no-launch concern is replaced by actual launched-gate results. Source/lock/sibling inputs remained stable; no implementation fix, assertion change, retry-to-green, map/spec/07 edit or accepted feature qualification. Final documentation validation uses `git diff --check`; commit hooks are disabled and the current integration tip is merged before handoff.
