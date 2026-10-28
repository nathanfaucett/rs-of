# Ticket 11 — bounded single-device matrix and CRAP validation (2026-10-28)

## Disposition

**Ticket 11 remains open. Neither final gate completed.** One authorized root feature-matrix attempt completed **4/10 combinations successfully**, then the unchanged 7200-second supervisor ceiling interrupted combination 5; combinations 6–10 never launched. One separate workspace `just crap` attempt completed its SDK prerequisite, then stopped safely during instrumented compilation when available RAM fell below 2 GiB. No compiler error or assertion failure was observed before either stop. This is incomplete qualification, not a passing matrix or a demonstrated new application-code blocker. There was no retry-to-green.

The [current spec](../spec.md#current-acceptance-scope-2026-10-28) permits sequential independent scenarios. All existing per-scenario concurrency, features, assertions and request/readiness deadlines were retained. Sequential success does not repair or supersede historical default-concurrency failures under [09](../issues/09-diagnose-readiness-and-request-timeouts.md). Ticket07's complete sequential owner **44/0/6** and unified Join **3/3** receipts remain separately scoped evidence, not substitutes for either broad gate.

## Ownership, inputs and preflight

- Branch: `qualify/setup11-single-device-20261028-b`; new worktree: `/home/nathan/code/rust/aicacia/of/.worktrees/setup11-single-device-20261028-b`.
- Confirmed starting HEAD, requested integration base and merge-base: **`e0fb4c257f146b48a54a85470e2291883c68336e`**, branch `integration/setup-07-completion`.
- Read current spec, ticket11, tracker/triage guidance, glossary map, ticket07 single-device evidence, root manifest, SDK scripts and `justfile`; loaded `tdd` and `ponytail`. Existing public test seams only; no tests or implementation were authored. Before the first document edit, reported that no new code blocker was demonstrated; historical broad-gate failures remained unqualified, not presumed fixed.
- Complexity/risk/priority: low implementation complexity (two documentation paths), high identity/security qualification and resource risk, routine broad qualification priority. Exclusive heavy Rust slot; one gate at a time. No unrelated process termination, profile/feature/assertion/deadline edits, diagnostic overrides, cache deletion, real profile/provider/OS trust changes, main-branch changes or existing-worktree removal.
- The unchanged supervisor hard-codes the integration checkout as cwd. Nested-worktree manifests also depend on sibling paths. Execution therefore used `/home/nathan/code/rust/aicacia/of` only after verifying no tracked integration changes and byte equality with the owned worktree. Parent ownership is limited to spec/map/07 documentation, not execution inputs. Authored delivery is only ticket11 and this new evidence.
- Compared **1,298 tracked inputs**: all indexed paths under `crates/`, `tests/`, `src/`, `apps/`, `.cargo/`, `scripts/`, `packages/`, plus root non-Markdown files. Symlinks are compared by link text, not followed into dependency directories. Sorted digest is SHA-256 over each UTF-8 path, NUL, then its binary SHA-256 content/link-text digest: **`2f0dddd557738c1f4747735b92cbf968d566b93123751cd8d92e76746ac211b7`**. Zero differences initially and between gates; same digest after narrowly restoring the recipe-generated source map below. Manifests, tests, production sources, root lock and supervisor were not edited.
- Initial integration status: only preexisting untracked `.worktrees/`. Siblings initially had no tracked changes; `ofdb` already had untracked `.worktrees/`. All sibling HEADs remained fixed. A later unknown sibling lock change is explicitly preserved below; blanket sibling-file immutability is **not** claimed.

| Repository | HEAD | Initial Cargo.lock SHA-256 |
| --- | --- | --- |
| of | `e0fb4c257f146b48a54a85470e2291883c68336e` | `a39e0f5211fb541b56d9e6cd8c498b2e9ce330c8ca2281c80cd4205ef5b532d1` |
| ofdb | `6286ca900f7a3fe73fa00e233656afab48d1fb81` | `7bb748d9a80168782fd632f30f98edf1c4ee6f82dcd33c032d8b1652eb5db0c5` |
| ofnet | `dbea925a0a3f99053fffc02618baccc2a1cbea9f` | `c3b5faf7fbc54c6e0b93ccca8dac24265507874fe94c688082acb458eef4f83d` |
| offs | `df4c696a910296635473ec296d9235acbaa60d9b` | `77f99f5bd2ceacb1e35c69c6cda36b102a995d1c817a9f095cf9a9f036693e40` |

Tools: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)`, cargo-hack `0.6.44`, cargo-llvm-cov `0.8.7`, cargo-crap `0.5.0`, just `1.49.0`, Node `v26.5.0`, pnpm `12.4.1`.

Each preflight sampled resources for 22 seconds. Matrix preflight minimum available RAM **7,055,060 KiB**, disk **367,595,790,336 bytes**, max memory PSI some/full **0/0**; CRAP preflight minimum RAM **5,548,284 KiB**, disk **339,891,286,016 bytes**, PSI **0/0**. Initial free swap was only **184 KiB**; swap exhaustion alone was not a launch veto. Adequate measured launch headroom did not guarantee sustained headroom: CRAP later hit the RAM guard.

## Recipe semantics, scheduling and reproduction

Inspected and reused `target/setup07-completion/run.py` unchanged, SHA-256 **`723d9bea4a1efdface8ddd9fffe5dcc90022c6bc135b6113b81621807971b32b`**. Unique labels were checked absent before launch; prior receipts were not overwritten. Each detached launch was followed by polling its **final `.json` and actual `exit_code`**, not interpreting launcher success as a gate pass.

The supervisor sets `CARGO_BUILD_JOBS=1`, `RUST_MIN_STACK=67108864`, shared `CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target`. It clears `RUST_TEST_THREADS`, `RUST_LOG`, `TOKIO_WORKER_THREADS` and both setup diagnostics. Neither Rust flags variable was set. It samples available RAM/disk, owned process-group RSS and memory PSI once per second, and stops its own workload group at RAM below **2 GiB**, disk below **30 GiB**, PSI `some avg10 >= 20` or `full avg10 >= 10` sustained **20 seconds**, or elapsed **7200 seconds**. It uses TERM, bounded wait, then KILL only if needed. RSS is sampled owned-group aggregate, not application memory or whole-machine consumption.

Actual commands, from the integration checkout, in order:

```sh
python3 target/setup07-completion/run.py launch ticket11-matrix-single-device-20261028-b cargo hack test --feature-powerset --all-targets -- --test-threads=1
python3 target/setup07-completion/run.py launch ticket11-crap-single-device-20261028-b env RUST_TEST_THREADS=1 just crap
```

**Scope distinction:** the exact matrix command selects root package `idp-unified`, **not `--workspace`**. All selected root targets run for each feature combination; it is not `just test`, whose recipe adds `--workspace`. Existing feature-required targets are naturally absent when their required features are disabled; no new filtering was introduced.

`just crap` first runs `sdk-build` (`pnpm sdk:build`, four existing owner-client ESM TypeScript builds), then exactly:

```sh
RUST_MIN_STACK=67108864 cargo llvm-cov --workspace --lcov --output-path /tmp/lcov.info && cargo crap --workspace --lcov /tmp/lcov.info
```

It is **workspace coverage with normal/default recipe features**, not a workspace feature powerset and not an `--all-targets` coverage command. Extra recipe arguments would reach `cargo crap`, not the test harness. Because the supervisor clears inherited `RUST_TEST_THREADS`, the child command explicitly uses `env RUST_TEST_THREADS=1 just crap`; this supplies serial libtest scheduling to coverage without modifying recipe semantics. The receipt's `observation_environment.RUST_TEST_THREADS: null` describes the supervisor environment **before** its child `env`, not the environment of `just`/coverage. Internal Tokio/owner coordination is unchanged. Coverage never reached test execution in this attempt.

Owner SDK exports already existed at their declared `packages/<owner>-client/esm/index.js` paths; the matrix's built-JavaScript owner-route test passed in all four completed combinations. No separate SDK rebuild/install was needed before matrix execution. The actual `just crap` prerequisite later rebuilt all four successfully. An exploratory check of `dist/esm` paths found nothing; package declarations confirmed that was not the artifact location or a missing prerequisite.

## Actual root matrix outcomes

Started **2026-10-28T12:39:55Z**. Final receipt: **`exit_code: 1`, `stop: "7200s build ceiling"`, elapsed 7201.005s**. No `FAILED` assertion result or compiler `error[...]` appeared before the stop. The supervisor limit, not a reproduced single-device code failure, ended the gate.

| Combination | Features selected by cargo-hack | Actual result |
| --- | --- | --- |
| 1/10 | `--all-features` | Complete success; owner boundaries **44 passed / 0 failed / 6 ignored**, 1175.94s; root unit target 0 tests, process/system parent 1 pass / 9 existing ignores, unified identity composition 1 pass, service example 1 pass |
| 2/10 | `--no-default-features` | Complete success; owner boundaries **44/0/6**, 1166.03s; process/system parent **1 pass / 9 existing ignores** |
| 3/10 | `--no-default-features --features cli` | Complete success; owner boundaries **44/0/6**, 1171.52s; root unit target 0 tests, process/system parent **1 pass / 9 existing ignores**, unified identity composition 1 pass, service example 1 pass |
| 4/10 | `--no-default-features --features completions` | Complete success; owner boundaries **44/0/6**, 1248.45s; process/system parent **1 pass / 9 existing ignores** |
| 5/10 | `--no-default-features --features cli,completions` | Launched, compiled, root unit target 0 tests; interrupted during `owner_boundaries`, last unfinished printed case `native_catalog_device_restrictions_do_not_revoke_bearer_results`; **no completed owner-suite result** |
| 6–10 | Remaining cargo-hack combinations | **Not launched** |

The six owner ignores remain existing child entry points called by their parents; child passes are not additional independent suite totals. The process/system target similarly retains its existing isolated/opt-in child entries; this command is not a full opt-in system-suite pass. **4 successful combinations, 1 partial, 5 unrun is not matrix completion.** At observed serial owner durations, ten combinations plus builds exceeded the authorized two-hour budget; no ceiling or deadlines were increased, and no combinations were resumed in a second attempt.

## Actual workspace CRAP outcome

Started **2026-10-28T14:40:42Z**, only after matrix completion receipt, absence of its group, unchanged execution-input digest and fresh resource preflight. SDK ESM builds all reported `Done`. `cargo llvm-cov --workspace` began instrumented compilation (including sibling dependencies) but had not completed compilation or started test binaries when stopped. The last logged compilation was `tonic-prost v0.14.6`.

Final receipt: **`exit_code: 143`, `stop: "available RAM below 2 GiB"`, elapsed 1138.646s**. Last available-memory samples were **2,666,520 → 2,284,040 → 1,862,236 KiB**. No compiler/assertion failure was observed; no attribution of the whole-machine memory drop to the owned compiler is established. PSI, disk and time did not trigger this stop. No subsequent attempt was made, and no unrelated process was terminated to manufacture headroom.

**No workspace test result, fresh LCOV, coverage percentage, CRAP report or score exists from this attempt.** The recipe's `&&` prevented `cargo crap` from running after coverage termination. Existing `/tmp/lcov.info` SHA-256 stayed **`20da6f83a069377c56e67c6c0ba7b32a29b6ebf54b4bba41d2944dec106b1e2f`** before/after; it was not used as current evidence. The previous device-contract failure/repair history is not newly requalified by compilation alone.

## Resource and receipt integrity

| Gate | Minimum available RAM (KiB) | Minimum free disk (bytes) | Peak owned-group RSS (bytes) | Max sampled memory PSI some/full avg10 |
| --- | ---: | ---: | ---: | --- |
| Root matrix | 3,622,356 | 339,899,265,024 | 5,270,503,424 | 22.60 / 20.01 |
| Workspace CRAP | 1,862,236 | 337,529,614,336 | 977,801,216 | 1.26 / 1.26 |

Matrix PSI briefly exceeded thresholds but did not sustain the 20-second guard; its actual stop was the ceiling. Resource receipts are sampled observations, not peak-exact or runtime performance certification.

Raw log SHA-256:

- Matrix: **`3a0b5a222da54d67bd690fa0b6b6046c344a90a727542e01383bfa8b35ecb2da`**.
- CRAP: **`0d5e5c18e145d43fd682cbbb1519daee6d29f98e663d0f20a1c7647c4257d21a`**.

Disposable artifacts remain under `target/setup07-completion/` for both unique gate labels: `.json`, `.running.json`, `.log`, `.resources.jsonl`, `.supervisor.log`. The retained `.running.json` is a launch record, not evidence of a live process. Additional unique inputs/preflight/SDK-delta receipts use `ticket11-single-device-inputs-20261028-b`, `ticket11-precrap-inputs-20261028-b`, and `ticket11-sdk-generated-20261028-b`. This committed evidence records outcomes independently of disposable cache retention.

## Cleanup, provenance caveat and integration handoff

- Matrix workload PGID **410168**, supervisor PID **410157**; CRAP workload PGID **553797**, supervisor PID **553785**. Both workload groups and both supervisors were absent at final inspection. No manual unrelated-process termination; caches and prior receipts retained. Exclusive heavy Rust slot released after both gates and cleanup.
- The SDK prerequisite changed only tracked `packages/storage-client/esm/index.js.map` in integration: one mapping segment `AA0CpF` → `AAgDpF`; emitted JavaScript was unchanged. Saved its exact patch and hashes, asserted that sole delta against the clean source-equal worktree and rechecked current bytes, then restored only that known generated output. Baseline SHA-256 **`ec299cf71c10b3a0330a493df7047962e87b9fffa8feebc8443b1a34ec352f45`**, generated SHA-256 **`09c980cb544cea06083dbf5daa2631bee17494e2f1169d3de3ce745ce718ff68`**. Integration source/config/script digest then again matched the initial digest, with no tracked integration changes. This transient generated-output change is not an application-source edit or a claim of byte-immutable SDK build products.
- **Unknown sibling work preserved:** `offs/Cargo.lock` became modified after the clean between-gates check; its filesystem mtime is **2026-10-28T14:52:16.628227Z**, during the CRAP attempt. Diff includes platform/web dependency updates. Current SHA-256 **`f28802586dde51de1d0f6d0a634f32fa6ea85c748d00f65319c1dde9cd99bb84`**. Cause/owner is not demonstrated, so it was neither restored nor committed. Root Cargo.lock stayed fixed and is the integration workspace's dependency-resolution lock; an unchanged sibling HEAD does not imply this sibling lock remained immutable. This is an explicit provenance limitation, not grounds to overwrite unknown work or call the incomplete CRAP run a success. No sibling source/manifest change was observed.
- Before delivery ran `GIT_EDITOR=true git merge --no-edit integration/setup-07-completion` in the owned worktree: **Already up to date**; integration tip/merge-base remained **`e0fb4c257f146b48a54a85470e2291883c68336e`**. Only the ticket11 path and this new evidence are delivery paths. No spec/map/07/09 edits, branch switch in integration/main, or preexisting-worktree modification/removal.

## Remaining qualification limits

Ticket11 cannot be resolved: full root powerset and completed workspace coverage/CRAP remain unchecked. Completion requires a separately authorized execution plan with sufficient sustained memory and a time budget compatible with unchanged serial suite duration; this attempt did not retry, silently split/restart the matrix, skip features/assertions or extend limits. No code fix is justified by the two resource/budget stops.

Historical generated-SDK workspace Clippy findings remain separate and were not rerun or waived. Historical standalone setup HTTP failures retain their history; ticket07's later 24/24 HTTP contract receipt supersedes them only at that seam, not as a fresh frontend-wide result here. No new frontend-wide checks, workspace Clippy, full opt-in process suite, packaged desktop smoke, actual GUI/window, production secure provider, real OS trust/elevation, physical/WAN pairing or platform/release certification was performed. Backend and injected-platform success cannot establish those unavailable checks.
