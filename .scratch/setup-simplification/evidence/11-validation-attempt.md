# Ticket 11: validation execution attempt

## Disposition

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
