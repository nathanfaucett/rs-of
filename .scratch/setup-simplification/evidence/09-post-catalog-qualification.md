# Ticket 09: post-catalog unchanged-bound qualification attempt

## Disposition — 2026-10-27

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

| Observation | Actual value |
| --- | --- |
| Initial supervisor MemAvailable |3,967,272 KiB|
| Stop sample MemAvailable / minimum |**2,084,116 KiB**, below2,097,152 KiB floor|
| Guard |**available RAM below2GiB**|
| Supervised elapsed |662.571s|
| Command exit |**-15 (SIGTERM from supervisor)**|
| Peak sampled owned-group RSS |1,887,404,032 bytes|
| Owned RSS at stop sample |1,695,928,320 bytes|
| Minimum available disk |394,391,863,296 bytes|
| Maximum memory PSI some/full avg10 |1.94 /1.81|
| PSI at stop |0 /0|

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
