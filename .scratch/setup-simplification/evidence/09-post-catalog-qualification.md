# Ticket 09: post-catalog unchanged-bound qualification attempt

**Latest completed qualification:** after explicit resource recovery and an approved owner-target incremental quarantine, the unchanged build passed. The single ordinary full suite **failed38/6/6**; one separate same-artifact diagnostic also **failed38/6/6**. The feature matrix remains gated and was not launched. Historical build stops/failures below are preserved separately from these actual test results.

## Initial stopped attempt — 2026-10-27

**Resource-stopped during the single build. No current owner test executable was emitted and no owner suite executed.** Consequently this attempt supplies **no deadline-impact result** for the integrated four-to-one Client catalog reduction. It is neither a compiler/test failure nor a passing qualification. No ordinary rerun, diagnostic, matrix, coverage or CRAP execution followed the stop.

## Approved inputs and preservation

On existing `investigate/setup-current-profile` / `of/.worktrees/setup-current-profile`, loaded TDD/ponytail, checked AGENTS and the existing qualification contract. Worktree was tracked-clean. Before compilation, fast-forward merged the latest integration with `GIT_EDITOR=true` (hooks disabled):

- Root **`dd41049ee2e7d68efa5c71b9fab734e6a35756bb`**, incorporating Client observation `a71fed7dc` and the receiver migrations.
- ofdb **`6286ca900f7a3fe73fa00e233656afab48d1fb81`**, including read-only standalone SELECT completion and existing-transaction catalog reuse, in addition to the earlier four fixes.
- ofnet `dbea925a0a3f99053fffc02618baccc2a1cbea9f`; offs `df4c696a910296635473ec296d9235acbaa60d9b`.
- Cargo.lock SHA-256 **`a39e0f5211fb541b56d9e6cd8c498b2e9ce330c8ca2281c80cd4205ef5b532d1`**. Compared parsed lock contents with `89339620a`: identical package/version sets, exactly two added existing-package dependency edges for `idp-service` (`futures`, `ofdb-sql`), no removed edges or other package changes.
- `client_repo.rs` SHA-256 `0720ea13b2d6c82f7d65b8059ee6b884fe250846ca5759b79c5e0aae5a876dba`; create fixture `1e2c8450a96146f1f2a7cbe3d34378f68e35845fb1e1ee691fcbfad92cff799b`; Join fixture `6af2df1e1deeb34529fe8b0be557628af48bffd2b4c597bf76042abd7749e2aa`.

No source reset or implementation/fixture changes. Only this new evidence file is owned in this attempt; no spec/map/ticket changes, sibling edits, main/push operations, provider/trust actions or deadline/retry/profile/concurrency changes. The sibling `scan_layout` 512-table standalone32s/>18s result remains separately unbaselined and uninvestigated; it was not run or assigned a cause here.

### SDK prerequisites actually verified

All five existing fixture-required exports existed and were successfully imported with Node before the build:

- `packages/idp-client/esm/index.js`:370 exported names.
- `packages/management-client/esm/index.js`:173.
- `packages/device-client/esm/index.js`:267.
- `packages/storage-client/esm/index.js`:5.
- `packages/storage-client/esm/generated/index.js`:import succeeded.

No install, ESM build, generation or generated-file restoration was needed. Stored each export's SHA-256 in `sdk-exports.json` and reverified after the stop. The primary worktree's seven external SDK differences, including the symlink target, were fingerprinted and verified unchanged. No writes to them.

## Single build and actual resource stop

Exact command, cwd the profile worktree:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=1 RUST_MIN_STACK=67108864 cargo test --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-unified --all-features --test owner_boundaries --no-run --message-format=json
```

Exclusive Rust slot; one Rust command only. Existing supervisor example adapted into a new private directory, without overwriting earlier scripts. Per-second resource monitoring;2GiB MemAvailable floor,30GiB disk floor, memory PSI avg10 some>=20 or full>=10 for20 sustained seconds,7200s ceiling. Every poll bounded. No unrelated process was terminated, cache deleted, stack/profile changed or deadline raised. Full swap alone was not a veto.

`RUST_LOG`, `RUST_TEST_THREADS`, `TOKIO_WORKER_THREADS`, `OF_SETUP_TIMEOUT_TIMING` and `OF_SETUP_SECRET_LOCK_DIAGNOSTIC` were absent from the build environment. `RUSTFLAGS`/`CARGO_ENCODED_RUSTFLAGS` and ambient dev/test opt-level overrides were also absent.

| Observation                        | Actual value                                |
| ---------------------------------- | ------------------------------------------- |
| Initial supervisor MemAvailable    | 3,967,272 KiB                               |
| Stop sample MemAvailable / minimum | **2,084,116 KiB**, below2,097,152 KiB floor |
| Guard                              | **available RAM below2GiB**                 |
| Supervised elapsed                 | 662.571s                                    |
| Command exit                       | **-15 (SIGTERM from supervisor)**           |
| Peak sampled owned-group RSS       | 1,887,404,032 bytes                         |
| Owned RSS at stop sample           | 1,695,928,320 bytes                         |
| Minimum available disk             | 394,391,863,296 bytes                       |
| Maximum memory PSI some/full avg10 | 1.94 /1.81                                  |
| PSI at stop                        | 0 /0                                        |

Compilation progressed through the owners and `unified-server`; the last line was compilation of root `idp-unified`. The RAM guard—not a Rust error, test watchdog, PSI/disk guard, OOM report or7200s ceiling—stopped the owned build process group. No compiler error was observed before termination. Global available-memory exhaustion does not by itself identify a particular unrelated process as the cause.

Parsed the actual Cargo JSON log: **zero executable compiler-artifact records for `owner_boundaries`**. No current executable/hash can therefore be reported for these inputs. An older file with a matching-looking name is not a substitute and was not executed. No second build was attempted.

## Qualification gates not reached

- **Ordinary full default-concurrency suite: NOT RUN.** No current built artifact; zero tests executed in this attempt.
- **Separately labeled same-artifact timing/lock diagnostic: NOT RUN.** There was no ordinary test failure to localize and no current artifact.
- **Exact supported root `cargo hack test --feature-powerset --all-targets`: NOT RUN.** The prerequisite ordinary full suite did not run/pass; zero matrix combinations attempted.
- **Coverage/CRAP: NOT RUN**, as scoped.

The earlier13 Client repository controls and sibling work-count proof remain their own receipts; they are not pooled into an integrated deadline result. The last recorded full-workload readiness/Storage blockers remain unqualified on the new inputs. A successfully completed current build after adequate resource headroom is available is the immediate blocker to measuring their actual deadline impact.

## Receipts, cleanup and handoff

Private directory **`/tmp/setup09-post-catalog/`**, mode0700, files finalized0600:

- `build.json`:actual argv, cwd, all heads, source/lock hashes, environment, return/guard/resource summary.
- `build.log`:mixed Cargo JSON/output; SHA-256 **`87b6b2500a495f03ef8002eb65ab43d42b4a13fdd663e367d9d1769437d1a313`**.
- `build.resources.jsonl`:complete sampled resources/PIDs; final sample elapsed662.539s records the RAM threshold crossing.
- `run.py`:supervisor SHA-256 **`ee91cc823ffe347ddd4c27082ef7634a648125a5e9f5cc12de0ff73c069b37a1`**.
- `sdk-exports.json`, `external-diffs.json`:preservation fingerprints.

Verified **no remaining process in the owned build PGID/session4074416**, including compiler descendants. No test/diagnostic/matrix process was launched. Worktree sources/SDK exports remained unchanged after the merge; no generated restoration required. Final integration tip checked atdd41049ee before evidence commit/merge confirmation. Only evidence is committed on this investigation branch. Rust slot released; no owned background work remains.

## Explicit resource-recovery continuation — 2026-10-27

Parent reported that, under prior explicit user permission, it verified the executable identities of Rust analyzer PIDs3728316/3728726 and SIGTERMed only those processes. This agent terminated no unrelated process. On resumption both PIDs were observed defunct (`Z`) with zero RSS; MemAvailable was approximately12GiB, versus the earlier approximately2.3GiB resource baseline. `resource-recovery.json` records12,307,708KiB available at the source check. This is a genuine resource-prerequisite change, not a normal-test retry: the stopped attempt emitted no executable and ran no tests.

Confirmed integration stilldd41049ee; merge returned already up to date. Actual branch HEAD was evidence-only `2b20e40bc`. Every prior source/lock hash, all five SDK hashes, and all seven external SDK differences matched. Re-imported all five SDK exports successfully; output in `sdk-recovery-imports.log`. No source reset, generated write or cache cleanup.

Resumed the exact same all-features Cargo JSON build, jobs1/shared target/64MiB stack, from retained cache with unchanged2GiB/30GiB/20-second PSI/7200-second guards. Receipt label **`build-resumed`**. It reached linking, then **failed with exit101**,265.714s, **no resource stop**. Minimum MemAvailable9,724,524KiB; minimum disk393,993,510,912 bytes; peak sampled owned RSS2,384,560,128 bytes; maximum memory PSI some/full avg10 0.72/0.64.

Actual compiler diagnostic: `linking with cc failed: exit status: 1`; `rust-lld` reported undefined hidden symbols, including `owner_boundaries::owner_composition_tests::join::run_production_join`, `owner_boundaries::permission_http_tests::run_http_test`, test-local drop glue and anonymous LLVM symbols, then its error-limit stop. Twenty-one `rust-lld: error:` lines include that final limit message. This is a **linker failure**, not a deadline/test failure or renewed RAM stop. Zero current executable compiler-artifact records were emitted; no older executable was used. Ordinary suite, optional diagnostic, matrix and coverage still have not run.

Read-only investigation localized a referenced object to the existing owner-target incremental directory: `target/debug/deps/owner_boundaries-fbdb13f2c672f60d.axnwbwhuiz3vyrn7bpnrnus0v.1wbcu4i.rcgu.o` is the same inode as `axnwbwhuiz3vyrn7bpnrnus0v.o` in both the old and latest sessions of **`target/debug/incremental/owner_boundaries-1u8g86tof19cn`**. Stale/incomplete incremental reuse after the interrupted compilation is a plausible hypothesis, **not a proven cause**. No cache was deleted or quarantined, and no additional build was launched. A narrow quarantine of that one owner-target incremental directory, retaining dependencies and all other targets, is proposed for approval before another unchanged-input build.

Private resumed log SHA-256 **`e650291b9e28b614776cc3795d8c8dfa19edf2d24fcb8c727b481485ee41275a`**; full linker text in `linker-diagnostic.txt`; resource samples/argv/heads/hashes in `build-resumed.{json,resources.jsonl}`. Files remain0600. Verified the resumed owned build group exited completely. At that checkpoint the investigation was **not complete**, with no integrated deadline-impact result or gate success. The following section records the subsequent explicit quarantine approval and actual qualification outputs.

## Approved owner-target quarantine and successful unchanged build

Parent explicitly approved renaming **only** `of/target/debug/incremental/owner_boundaries-1u8g86tof19cn` to a unique sibling quarantine path, retaining artifacts/metadata and all dependency/other-target caches. Recorded the failed object and linker diagnostic before the rename:

- Referenced object `target/debug/deps/owner_boundaries-fbdb13f2c672f60d.axnwbwhuiz3vyrn7bpnrnus0v.1wbcu4i.rcgu.o`, inode10015846, SHA-256 **`ada54d491771cb308e8b3fd2178f2c1de9a94dbaddeacf107c539e3f3d1c2e3f`**.
- Full linker diagnostic text SHA-256 **`2f7fc07a13aa4b0ffafa005304d1ebdd536bbe94ae6e014b7c628f17472a75af`**, including the test-local hidden symbols described above. No source/flags/dependency repair.
- Renamed to **`of/target/debug/incremental/owner_boundaries-1u8g86tof19cn.quarantine-20261027T143159Z`**. Same directory inode after rename; no copy/delete replacement.
- `quarantine-manifest.json` records1008 entries with paths/inodes/modes/sizes/mtimes and file SHA-256s. Manifest SHA-256 **`6847de0c6b3ab427db8516a0a50faa4870b293d1ac5eae783f6fdc41cadb5d8b`**. Reverified all recorded contents/metadata unchanged after compilation and both test runs. The quarantine remains available; no dependency/other-target cache was removed.

Repeated the exact all-features Cargo JSON build with unchanged sources, lock, feature flags, target, jobs1, stack and resource guards. Label **`build-quarantine`**: **passed**,203.065s supervised /3m22s Cargo. This same-input success after the single scoped quarantine supports an incremental-artifact explanation for the failed link; it is not an application source fix or evidence of a particular on-disk bit-corruption mechanism. These build continuations preceded the first test execution and are **not ordinary test retries**.

Resolved exactly one matching current executable from Cargo's actual compiler-artifact record:

- `/home/nathan/code/rust/aicacia/of/target/debug/deps/owner_boundaries-fbdb13f2c672f60d`
- SHA-256 **`c95f55f3a0ba9be28a469acfe9471110dc80400cb911619c3254d60bc85a2543`**.
- Features `cli`, `completions`, `default`, `swagger-ui`, `unified-server`.
- Full artifact record in `artifact-current.json`. Rehashed before runs and after both; unchanged. Source/lock/SDK hashes also remained unchanged. Runtime code inputs are integrationdd41049ee/ofdb6286ca900; the branch HEAD during execution was evidence-only2b20e40bc plus this document's continuation, not a new code input.

## ONE ordinary full default-concurrency qualification — failed

Executed only that exact binary, **no arguments**, no filters, `--nocapture`, thread override, debugger or trace. Removed timing/lock diagnostic flags, `RUST_LOG`, `RUST_TEST_THREADS` and `TOKIO_WORKER_THREADS`; `RUSTFLAGS`/encoded flags absent. Both observation flags are explicitly null in `ordinary.json`. Fixture-internal child arguments/parent/request/readiness bounds remain unchanged.

**38 passed /6 failed /6 ignored**, exit101,224.17s libtest /224.358s supervised; no resource stop. This is the actual uninstrumented default-workload result on the integrated Client observation/transaction-catalog changes, not a subset pass or a diagnostic substituted for qualification.

| Failed public parent                                                                    | Actual ordinary blocker                                                                            |
| --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `create::production_independent_create_and_offline_reopen`                              | `create.rs:453`, authenticated Storage database-list request-send timeout                          |
| `create::production_independent_create_unsupported_record_refusal_and_current_reset`    | Same authenticated Storage send timeout                                                            |
| `operation::production_independent_signer_free_operation_and_delayed_authority_reopen`  | Same authenticated Storage send timeout                                                            |
| `remote_setup::remote_create_join_reopen_and_device_reset_preserve_shared_installation` | **`create.rs:453`, authenticated Storage send**, not the earlier remote Storage-readiness blocker  |
| `join::production_independent_join_and_source_offline_reopen`                           | **`create.rs:260`, bounded independent Storage readiness**, not IdP readiness in this ordinary run |
| `join::production_independent_join_unsupported_record_refusal_and_current_reset`        | Same bounded independent Storage readiness                                                         |

Both ordinary Join failures therefore advanced past the IdP readiness gate; remote setup advanced to actual authenticated Storage use. The integrated changes have **not** made the full workflow meet its unchanged budgets. This run is not a controlled A/B allocation of milliseconds to the Client four-to-one change alone: sibling read-only completion/catalog reuse, root lookup changes and recovered host resources all differ from older receipts. No normal-to-green replay was attempted.

## ONE separate same-artifact diagnostic — failed, not qualification

After preserving the ordinary failure, announced/ran exactly one separate full default-concurrency diagnostic using the same executable bytes and no arguments. Enabled only:

```text
OF_SETUP_TIMEOUT_TIMING=1
OF_SETUP_SECRET_LOCK_DIAGNOSTIC=1
```

No thread override, nocapture, GDB, trace or new observation code. **38 passed /6 failed /6 ignored**, exit101,265.91s libtest /266.712s supervised; no resource stop. One85-second polling tool call timed out after displaying partial status; the detached supervisor/workload continued unchanged and completed normally with the recorded test failure. It was not restarted and no test/tool ceiling stopped the workload.

The same four public parents fail their authenticated Storage sends. **Both diagnostic Joins fail IdP readiness instead of the ordinary run's Storage readiness.** Their IdP lifecycle remains `joining` during the failing wait, with no current-readiness callback records for PIDs4114187/4114193. Do not describe these as the older successfully-completed-but-late current-readiness checks. Their logs show successful IdP history import/retained-signer work (public-import31,916/32,821ms), local-client seed8,085/6,931ms, and serving270/264ms; those phases do not establish lifecycle ready. No deadlock, bad signer, missing history or database-admission cause is inferred from this boundary. Diagnostic variability does not replace or explain away the ordinary result.

Coherent current IdP observations elsewhere in the diagnostic:

| PID                    | Complete current check, ms                         | Initial relationship / subsequent relationship starts, ms                |
| ---------------------- | -------------------------------------------------- | ------------------------------------------------------------------------ |
| 4114065 / Create       | 1998                                               | 585 /957 /1307 /1657                                                     |
| 4114079 / Create-reset | 2012                                               | 520 /877 /1222 /1661                                                     |
| 4114257 / operation    | 2246                                               | 758 /1117 /1526 /1883                                                    |
| 4114382 / remote       | one coherent completion4206 among8 coherent starts | Partial/cancelled sequences not converted into invented completion times |

Four failing authenticated Storage handler-end observations are5006/5001/5001/5023ms. End guards alone do not distinguish success from cancellation; actual request-send failures and five-second client budgets are retained, not relabeled successful responses.

Thirty-two ordered complete client-credentials sequences, keyed by PID/thread and excluding corrupted/interleaved/incomplete lines, have phase median/max milliseconds:

| Phase               | Median | Max |
| ------------------- | -----: | --: |
| Client lookup       |     53 | 123 |
| Secret verification |  364.5 | 589 |
| Grant validation    |      0 |   1 |
| Active root         |     86 | 192 |
| Principal           |  221.5 | 334 |
| Issuance            |  419.5 | 666 |

These observations differ from prior diagnostic medians (lookup169, principal338, issuance636ms), but are not controlled per-fix latency measurements or an allocation of the outer deadline. The remaining nested authorization path still overruns unchanged requests. No new native CPU/admission-wait profile was collected here.

Setup-operation diagnostics recorded **1794 acquired /1794 released /zero contended** events. This is not a database-admission wait measurement or a proof that historical contention cannot recur. Independent permission and the other passing tests are not pooled into a full-workflow pass.

## Final gates, resources and handoff

**Exact root `cargo hack test --feature-powerset --all-targets`: NOT LAUNCHED**, because the ordinary full prerequisite failed. Zero matrix combinations attempted in this continuation; no full feature qualification, coverage or CRAP result. The sibling512-table timing failure remains separately unbaselined/out of scope. Current exact blockers are the ordinary run's four authenticated Storage send timeouts and two Storage-readiness failures; the diagnostic additionally exposes Join lifecycle/readiness variability described above.

| Continuation stage                    | Exit | Supervised s | Minimum available KiB | Minimum disk bytes | Peak sampled owned RSS bytes | Max memory PSI some/full avg10 |
| ------------------------------------- | ---: | -----------: | --------------------: | -----------------: | ---------------------------: | -----------------------------: |
| Recovered-memory retained-cache build |  101 |      265.714 |             9,724,524 |    393,993,510,912 |                2,384,560,128 |                      0.72/0.64 |
| Build after approved quarantine       |    0 |      203.065 |            10,250,496 |    392,835,887,104 |                5,548,879,872 |                            0/0 |
| Ordinary full suite                   |  101 |      224.358 |            10,568,908 |    392,977,936,384 |                  928,759,808 |                      0.18/0.18 |
| Separate diagnostic                   |  101 |      266.712 |             8,740,656 |    392,966,623,232 |                  863,551,488 |                      1.31/1.31 |

All continuation stages used the unchanged2GiB RAM/30GiB disk/sustained20-second PSI20/10/7200-second guards; none fired. No additional unrelated process termination, source/dependency/flag change, cache deletion, generated SDK write/restoration or broad concurrency change. Parent-performed analyzer termination is recorded separately above and is not attributed to this agent.

Private continuation receipts under `/tmp/setup09-post-catalog/` retain actual argv, environments, heads/source hashes and resource samples. Log SHA-256s:

| Log                    | SHA-256                                                            |
| ---------------------- | ------------------------------------------------------------------ |
| `build-resumed.log`    | `e650291b9e28b614776cc3795d8c8dfa19edf2d24fcb8c727b481485ee41275a` |
| `build-quarantine.log` | `09a1bc152448321ad51c4024b0cc47ac60ed120951160063866ed928b1d050d6` |
| `ordinary.log`         | `5e15dbe4607c95324064b79d66d1bcb81ec02b12b6335bea575598ef2b180ef5` |
| `diagnostic.log`       | `d493ebca3bcbb607c40c0418ab396b32b49afeec7442522c188ff80acc839ba8` |

Final verification: source/lock/SDK hashes and all seven external SDK difference fingerprints unchanged; quarantined1008 entries retain recorded contents/metadata; executable hash unchanged. Private files0600. All owned build/test PGID/session groups exited. Integration tip stilldd41049ee before final evidence commit/merge confirmation. Only this evidence continuation is committed, no spec/map/ticket/source/sibling/main/push changes. Rust slot released after collection. This records **completed failed qualification gates**, not a finished feature, green matrix, or closure of ticket09/11.
