# Ticket 09: current readiness and Storage authorization native profile

## Scope and immutable inputs — 2026-10-27

User-approved execution on `investigate/setup-current-profile` in `of/.worktrees/setup-current-profile`, based on root **`30b32698a86e069ce54b49936492fad619507c54`**, including the reviewed explicit setup-operation unlock/fork-inheritance fix. Read root AGENTS, shared glossary, setup spec and prior timeout/Storage evidence; loaded TDD and ponytail. This is observation at the existing public `owner_boundaries` seams, not a behavioral red/green implementation.

Sibling inputs throughout compilation/execution:

- ofdb **`a8c23a54540e591aa024396a5674dc016b6cce3a`**: candidate batching, exact index-field point reads, schema execution-stage stack boundaries, and same-transaction index-update schema reuse are all present.
- ofnet `dbea925a0a3f99053fffc02618baccc2a1cbea9f`.
- offs `df4c696a910296635473ec296d9235acbaa60d9b`.
- Root Cargo.lock SHA-256 `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`.

**Only this new evidence file is changed.** No production/observation Rust changes, key batching, sibling edits, deadline/retry/stack/profile changes, cache, broad concurrency change, ticket/map/spec edits, or real provider/trust work. Complexity low for evidence, high attribution/security risk: keep measured stacks separate from proposed repairs and keep response reauthorization/current readiness intact.

## Supervision, SDK prerequisites and actual build

Exclusive Rust slot; `CARGO_BUILD_JOBS=1`, shared target `/home/nathan/code/rust/aicacia/of/target`, `RUST_MIN_STACK=67108864`. No user process was terminated. At entry MemAvailable was about 5 GiB; the two existing analyzers consumed about 8.4 GiB RSS. Full swap was not used as a veto. Guards: MemAvailable below 2 GiB, free disk below 30 GiB, memory PSI avg10 some >=20 or full >=10 continuously for 20 seconds, 7200-second build ceiling. Only owned process groups/descendants were eligible for cleanup. No guard fired.

Private supervisor copied/adapted, without overwriting, `/tmp/setup-lock-inheritance-receipts/run.py`; native profile adapted the prior ancestor-GDB method in `/tmp/setup09-seed-profile.py` and `.gdb`. Receipts/scripts reside in **`/tmp/setup09-current-profile/`**, directory 0700, files finalized 0600. The directory-creation tool did not support `/tmp`; creation used Python instead. All long-running work was supervised and polled with bounded tool calls.

One exact build, no rebuild/retry:

```sh
cargo test --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-unified --all-features --test owner_boundaries --no-run --message-format=json
```

Build **passed**, 1073.180s. Resolved the executable from the single matching Cargo JSON compiler-artifact, not a stale filename:

- `/home/nathan/code/rust/aicacia/of/target/debug/deps/owner_boundaries-fbdb13f2c672f60d`
- SHA-256 **`68ba1c045f7ccb02570c765a6ff73582f25f20d986da0231ba95dca547a355c0`**.
- Features `cli`, `completions`, `default`, `swagger-ui`, `unified-server`.
- Artifact rehashed before execution and after both runs; unchanged.

SDK prerequisites were absent in this worktree. After the Rust build, ran supervised `pnpm install --offline --frozen-lockfile --ignore-scripts` (passed, 1.086s, 465 reused packages, zero downloads), then `pnpm sdk:build` (all four ESM builds passed, 3.096s). Verified all five fixture-required exported files and successfully imported all four ESM SDK entry points with Node. Before installation, backed up the 16 tracked generated SDK paths. Restored only the eight generated tracked changes caused by this installation/build, against that clean-worktree backup; worktree tracked status was clean before tests. No source generation or dependency/lockfile change. The primary worktree's **seven external SDK diffs** were fingerprinted, including the symlink target, and verified byte/target-identical afterward; they were never restored or edited.

| Stage | Exit | Seconds | Minimum available KiB | Peak sampled owned RSS bytes | Max memory PSI some/full avg10 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Build | 0 | 1073.180 | 2,634,084 | 4,828,909,568 | 7.95 / 7.10 |
| SDK install | 0 | 1.086 | 5,390,708 | 7,995,392 | 0.38 / 0.38 |
| Four SDK ESM builds | 0 | 3.096 | 4,791,040 | 53,833,728 | 0 / 0 |
| Full diagnostic | 101 | 241.366 | 4,492,780 | 909,430,784 | 0 / 0 |
| Ancestor-GDB profile | 0 | 131.185 | 2,909,984 | 3,286,671,360 | 0.92 / 0.92 |

Build minimum free disk 412,371,939,328 bytes; diagnostic 412,333,658,112 bytes. SDK and GDB disk samples also remained far above the floor. SDK one-second RSS sampling can miss short process peaks; no claim of exhaustive peak accounting.

## One workload-restored default-concurrency diagnostic

Ran the exact executable above **without filters, thread override, nocapture, GDB or tracing**, with only these diagnostic flags enabled:

```text
OF_SETUP_TIMEOUT_TIMING=1
OF_SETUP_SECRET_LOCK_DIAGNOSTIC=1
```

`RUST_LOG`, `RUST_TEST_THREADS` and `TOKIO_WORKER_THREADS` were removed from the run environment. Jobs/target/stack retained as above. **38 passed / 6 failed / 6 ignored**, exit101, libtest240.43s / supervised241.366s. This is a diagnostic with optional observation, **not fresh uninstrumented qualification**, and not a rerun to green. Original 180-second parents, five-second requests and 60-second readiness waits remain unchanged.

Failures:

- Both Create parents and operation/delayed-authority parent: `create.rs:453`, authenticated Storage database-list send timeout.
- Remote setup: `create.rs:260`, eventual Storage readiness request ends at5000ms after earlier slow IdP attempts.
- Both Join parents: `create.rs:260`, IdP readiness.
- Independent permission parent passed in this diagnostic; this does not retroactively explain its older ordinary failure.

Coherent, single-PID current IdP checks:

| PID / scenario | Complete current checks, ms | First initial-relationship / subsequent starts, ms |
| --- | --- | --- |
| 3933075 / Create | 3157 | 766 / 1364 / 1993 / 2580 |
| 3933090 / Create-reset | 3258 | 863 / 1463 / 2075 / 2681 |
| 3933135 / operation | 3205 | 798 / 1406 / 1980 / 2562 |
| 3933115 / Join | 5676, 6106 | 1437 / 2519 / 3533 / 4596 |
| 3933125 / Join-reset | 5819, 6209 | 1470 / 2571 / 3657 / 4749 |

Join current checks still complete successfully **after** the five-second client budget. These are not missing signer/history evidence or a still-joining worker. Remote PID3933179 begins nine coherent checks, only one coherent completion3945ms; partial sequences are not turned into invented completion times. Interleaved/corrupted lines were excluded from phase accounting.

Create Storage request-end observations are5002/5000ms; operation4998ms. End guards alone cannot distinguish success from cancellation; these match actual send-timeout failures, not successful responses.

Thirty-four ordered, complete, coherent client-credentials sequences keyed by PID/thread have phase median/max ms: client lookup169/222; secret verification306.5/458; grant validation0/0; active root85/123; principal338/398; issuance636/788. This still does not support password-only offload as the whole bottleneck.

Setup-operation diagnostics: **1794 acquired /1794 released /0 contended** records. These are the `setup_secret_operation` seam, not database-admission events. They do not identify the historical WouldBlock holder or prove contention is impossible.

## One separate late-path native profile

Used the permitted exact-parent fallback rather than attach to an arbitrary child of the fully concurrent workload. Ancestor GDB launched the same executable with:

```text
--exact owner_composition_tests::create::production_independent_create_and_offline_reopen --nocapture
```

Both diagnostic flags remained enabled. GDB followed the fixture fork/exec into `independent_create_child`, then restored follow-parent behavior; detached the watchdog parent. No external ptrace attach, process arguments, frame arguments, locals, payloads, broad HTTP logging or debugger auto-load scripts. Debuginfod and auto-load were disabled. A single entry breakpoint established child PID3933888. External interrupts began only after current-readiness `idp_readiness phase=record`, or while Storage `databases_start` lacked a later matching `databases_end`; **not schema-open/semantic seeding**. Caps12 readiness and28 Storage snapshots, minimum250ms interruption spacing. Every stop collected frame **names only**, maximum180 frames/thread, then immediately continued. Forty snapshots /272 thread stacks collected. No debugger errors or resource stops.

Parent and child passed (122.51s /121.94s test bodies), supervised131.185s, debugger exit0. **This isolated sampled pass is not qualification and does not cancel the six default-workload failures.** The windows include both initial authorization-negative Storage calls and actual successful authenticated use, then offline-reopen checks. They are explicitly separated below.

### Work actually caught on native stacks

Across40 gated snapshots:

- **33** contain a catalog traversal stack: **28 field catalog** (`schema::read_columns`), **5 table catalog** (`schema::read_table_id`).
- Of those33, **18** are under `executor::row_conflicts_batch`, **15** under `executor::select_rows`.
- **17** snapshots catch `btree_automerge::reconstruction` frames; these overlap catalog counts, not an additional17 samples.
- **2** catch Argon2 client-secret verification.
- **4** catch durable `fdatasync` through redb commit. Three retain explicit read-only client-registration SELECT callers; one lacks that owner context in the captured depth.
- One snapshot is outside those classified buckets. No sampled async-lock mutex acquisition stack. Sleeping Tokio/driver threads are not classified as useful work.

These are **stack-hit counts, not function invocation counts, work-count regression assertions, unbiased CPU percentages or proof that unobserved waits never occurred**. Gating, debugger overhead, frame-depth truncation and the isolated topology limit attribution.

Representative actual chains (generic/future-poll scaffolding omitted):

1. Samples3/4: `verify_setup_infrastructure_registration → DbClientRepo::find_client_by_client_id → ensure_clear → row_conflicts_batch → read_table_id/read_columns → catalog_states_range → reconstruct_document_values → ReconstructedDocument::apply → AutoCommit::load → Document::reconstruct / hexane`.
2. Samples7/8: relationship check → `DbApplicationRepo::find_by_id → find / ensure_clear → SELECT / row_conflicts_batch → read_columns → catalog_states_range`.
3. Sample9: relationship check → `ensure_client_root_locked → verify_local_entity_root → DbKeyRepo::find_active_entity_root_key → list_by_entity_type_and_id → ensure_clear → read_columns → catalog_states_range`.
4. Samples12/13: `client_credentials → find_principal` and later `issue_client_credentials_access_token → validate_issuance_inputs → find_principal → principal_for_active_key → DbClientRepo::find_client_by_id → registrations / clients → read_columns → catalog_states_range`.
5. Samples14/26: bearer verification → `principal_for_claims → DbKeyRepo::find_active_entity_root_key → list_by_entity_type_and_id → keys → SELECT → read_columns → catalog_states_range`.
6. Samples15/19/20/24: bearer verification → `find_public_jwk → trusted_node_signers → read_columns → catalog_states_range`. Existing signer-candidate conflict batching is present; these are remaining catalog reads, not a recommendation to repeat that fix.
7. Samples21/22/25: nested current User work → `find_user_info / hydrate_user_info → catalog_states_range`; samples31/35 catch the issuance revalidation path again after offline reopen.
8. Samples2/10/18: read-only `DbClientRepo::registrations` SELECT completion → `NativeWriteTransaction::commit → WriteTransaction::durable_commit → fdatasync`. Durable read-transaction commit overhead is a distinct measured contributor, **not database admission wait**.

### Native CPU versus debugger and elapsed time

Supervisor sampled each owned native thread's `/proc/.../task/.../stat` user/system ticks at about100ms intervals; `SC_CLK_TCK=100`. Values below sum child thread ticks separately from debugger thread ticks using nearest boundary samples. They are coarse process-wide CPU observations, not individual-future CPU accounting. Clock-bracketing errors for the first three rows are at most61ms. Concurrent CPU and wall time are not assumed strictly additive.

| Interval | Wall | Child CPU | GDB CPU | Native snapshots |
| --- | ---: | ---: | ---: | --- |
| First current-readiness start to completion | 5.714s | 1.31s | 4.25s | 0–9:7 catalog,2 fdatasync,1 other |
| Authorization-negative Storage call | 1.022s | 0.80s | 0.23s | 10–13:2 catalog,1 Argon2,1 fdatasync |
| **First successful authenticated Storage list** | **3.527s** | **2.56s** | **0.85s** | **14–26:11 catalog,1 Argon2,1 fdatasync** |
| Later Storage list after reopen | 3.695s | 3.26s | 0.29s | 29–39:11 catalog |
| Subsequent Storage request, no further interrupts | 2.251s | 2.24s | 0.00s | none |

The first profiled readiness request exceeds five seconds largely because debugger CPU dominates its elapsed window; another current check starts at5.053s, before the earlier check completes. Do not attribute all that interval to one serial native check or use it as an untraced readiness failure. The full diagnostic's coherent Join5.676–6.209s observations are the independent current lateness evidence.

The successful Storage window is the third initial database-list request in `create::normal_use` (the earlier two are authorization negatives). It reaches the same real Storage route and nested current IdP/Device work that times out in the default workload. CPU remains high even in a later window after the40-stop cap has been exhausted. This localizes substantial actual execution, rather than an inference from source alone. Short DB-admission/I/O/scheduling waits remain possible; no multi-second admission wait was established. The fdatasync samples separately establish synchronous durable-commit work.

## Owning next boundary and limits

The supported next causal target is **repeated complete field-catalog traversal/reconstruction in ordinary SELECT and conflict observations**, reached by readiness relationship validation, current bearer/principal resolution, and issuance revalidation. Actual stacks now distinguish that from Argon2-only cost and from setup-operation-lock contention. Also retain durable SELECT commit cost as a separately measured contributor; do not conflate it with the catalog CPU or change transaction semantics without a dedicated regression.

No exact-key SELECT, reconstruction shortcut, invalid-catalog skip, multi-call snapshot/cache API or lock/concurrency change was implemented. `reconstruct_document_values` remains one streaming reconstruction per supplied document/change range; native `AutoCommit::load` hits demonstrate expensive repeated observations, not an internal duplicate-heads bug. Existing field point-read/schema-local stack/index-schema fixes remain integrated, not reopened. **Do not implement per-key batching for this singleton fixture.** All freshness, malformed catalog/row refusals, conflict checks and response reauthorization remain unchanged.

No uninstrumented qualification, feature matrix, coverage, CRAP or Clippy was run in this profiling-only scope. No green whole-workflow claim. Ticket09 remains unresolved; the ordinary integrated qualification gate is still open.

## Receipts and handoff

Private command/environment/head/source hashes and resource samples are in `/tmp/setup09-current-profile/{build,sdk-install,sdk-build,full-diagnostic}.{json,log,resources.jsonl}`; executable/compiler JSON is `artifact.json`; native data in `late.log`, `late-resources.jsonl`, `late-result.json`; CPU boundary calculations in `phase-cpu-summary.json`. No raw fixture payloads/log transcripts are committed.

| Receipt | SHA-256 |
| --- | --- |
| Build log | `5259c6ec5ebe8134f490e1b14332e57942853406597004afac7e182da96389fa` |
| Full diagnostic log | `f7bdf25f21ceea169c7e6cbf0f51c8393e0a86cf28cbe895d2483ec5dc9c573b` |
| Native profile log | `fbca068848f39a9c63f04a4ad0b6a5d96e9113044521cf4d4d58c6e57fc529a5` |
| Supervisor | `5a7c48e582643a3343c22e91195ab5d1166a52116d2cc98df63b25840710c314` |
| Late profile supervisor | `ff9fe46a3c86196c22c9eb925c287da37991e7b15d5dacf2985f8d1d3eb12124` |
| Late GDB commands | `e8cd823e3b851e7581dad13832ca6108893241c18915ff3c87c363764daf4ed0` |
| CPU boundary summary | `7ed8034022a761b12855564130b9e242dc0f41ef6757f7805899bbe57def47cc` |

All owned run groups exited after collection. Final artifact hash and external seven-diff fingerprints checked unchanged. Integration tip checked at `30b32698a` before handoff; merge/confirmation and evidence commit belong to this investigation branch only. Rust slot released after final checks; no background qualification work remains.
