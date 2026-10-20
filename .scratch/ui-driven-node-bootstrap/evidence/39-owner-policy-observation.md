# Receipt 39 — Transaction-local Management owner-policy observation

## Outcome

**Management rewrite implemented; stable independent Join qualification is still incomplete. No commit made.** The unchanged source Join first catalog request still intermittently exceeds the five-second HTTP bound. A complete source Join and historical Join run passed, but the repeat failed source Join at that same first request. Historical Join passed twice. These are not fixture-deadline failures, and a passing attempt is not being substituted for the failing repeat.

Receipt 38 was read first. Its measured Storage failure was reproduced before implementation, through `production_independent_join_and_source_offline_reopen`, not a substitute bundled/unified catalog fixture. Neither the fixture nor its first catalog request was changed.

## Source and ownership

- MAIN branch `integration/ui-driven-node-bootstrap`, HEAD `9c4de8db1feb16d3377d2305d280f58283ac7493`, unchanged throughout.
- All 14 initially dirty Rust files, the existing deleted `evidence/review-transfer-lifecycle.md`, and prior evidence remain parent-owned. Changes in this turn are confined to four Rust files listed below and this receipt. No TS, generated clients, other docs, external sources, manifests, lockfiles, or generic persistence implementation were edited.
- Final combined working-tree Rust diff SHA-256 (`git --no-pager diff --binary -- '*.rs' | sha256sum`): `50964d97952fbebe7597f8307c3153deefbe469ee4572202ee9d1f200da67109`.
- External `ofdb` HEAD remains `238210c5b822cd8b5f14274ea63d941ef884f6a0`; tracked dirty diff SHA-256 remains `0827bcc5f65fd8d10ffc861744e076a4d6a53007d7d9d25285db057cf5cbe26b`. Its existing untracked `crates/btree/src/sharded_btree.rs` is not included in that hash and was not edited.
- No staging, commit, ticket closure, or branch creation. The runtime acceptance gate is not stable, and the observation implementation also depends on the preserved parent-owned cleanup-history schema/query changes; this receipt does not certify an own-only standalone commit against clean HEAD.

### Own Rust file provenance

| File | Initial file SHA-256 | Final file SHA-256 |
| --- | --- | --- |
| `crates/management-service/src/replica/selection_policy.rs` | `d2f841cdd7592c2b1c5f3c8f1a7950e2182b6498d53d7284fc0f259b9a03a41e` | `2d1db461240f2c1d9b1eb600a2c18283ec31475ddf0b9809ea126edf6d6aa43c` |
| `crates/management-server/src/router/routes/replication.rs` | `314b8af4dfbc9ba58915bb50f62532121d61cd7d7c51e31960c9d5fcda0027df` | `7df93f1ddb01476f5397aff79104daaa91e9f830f86839fc5f97a19761d81093` |
| `crates/management-service/src/replica/mod.rs` | `0e8bd13e5b4867a948a5c4923e2d87b48a9788cac2ae806d4d8eb250aa7919b3` | `b1a871b45c50949cb5285b756da7f11e1eb6e83c332a4a30a1934a4c78c8f135` |
| `crates/management-server/src/runtime.rs` | `d7b999b3d2a596bd209c3980ed01e5d353b0c9091d768d1670d251c02c2b17ce` | `22e7e7656a0f0de256b4d9052e63af612738f55512cf54853320558f1b002c6e` |

Initial versions of the two already-dirty files were saved before editing; the other two were initially HEAD-clean. SHA-256 of the concatenated Python `difflib.unified_diff` own deltas, in table order, labels `initial/<path>` and `current/<path>`, no timestamps: `8609626402f4e0d584cf57f03ee7fdaa640f8ae608bbdc0ae19de4f5c532efbc`.

## Boundary rewrite

Before: selections route reconciled ownership, independently fetched pending cleanup fields or called runtime; runtime reconciled again, called `get()` (which resolved/check/read/checked the already-read policy), then independently queried/revalidated selected resources. The reconciliation transaction discarded the policy and returned a Boolean. This was concrete duplicated work, not evidence safe to reuse across separate observations.

After:

- `observe_owner_prevalidated()` returns `MissingPolicy`, `Ready { policy, resources }`, or `PendingCleanup { cleanup_id, old_owner_subject }`.
- Existing conservative policy conflict resolution runs before opening the observation transaction. The transaction checks the policy is conflict-clear again; conflict evidence from before the transaction is not trusted to authorize its read.
- The policy, completed-versus-pending cleanup history, and ready resource rows are read/validated within that same transaction. Resource row IDs must match their exact device/owner/application/kind/resource bindings. Unknown kinds and nil application/resource IDs refuse. Selected resources must match the supplied current owner.
- Resource conflict values are checked in that transaction without per-resource requery. Only a concurrent `selected=false` conflict can become denial evidence; that resource is omitted. Ambiguous binding/other conflicts refuse. Observation does not erase resource conflicts or persist a winner; the existing standalone resource-selection resolver remains unchanged. This is conservative transaction-local interpretation, not permission cached from a prior read.
- Pending cleanup returns only the durable cleanup ID and old owner, never ready resources. Transfer fencing and resource deselection commit atomically. Completed history is retained, not mistaken for pending history; a later transfer gets a fresh ID. A->B->A remains pending until cleanup. The existing exact-bound acknowledgement/replay logic is unchanged.
- Existing Boolean reconciliation callers now consume the shared observation, not a second reconciliation implementation. Missing policy still cannot authorize runtime/route publication.
- Runtime and Management selections route both consume the observation; neither does a policy/resource readback. Route response conversion is synchronous after observing policy.
- The Management route's FINAL `get_storage_endpoint_identity()` is retained after asynchronous work, including current owner-active validation and equality against the initial identity. Both Storage pre-access and response-time authorization paths are unchanged.
- No cross-request policy cache, deadline change, warming request, special preflight, artificial retry, or removal of checks. Repeating an entire qualification fixture below is test validation, not a production retry.

Risk assessment: localized implementation surface but security-critical behavior, so exact bindings, restrictive conflicts, transfer history/replay, and publication refusals were prioritized over further optimization. No generic persistence changes or broad CRAP/powerset sweep was attempted in this narrowly scoped token.

## TDD and focused behavior

`tdd` was loaded. The user-specified public seams were retained: the existing native runtime regression, public Management repository/HTTP contracts, and actual independent production Join.

1. The existing source Join regression failed before implementation at `independent_join_tests.rs:515`, `normal joined Storage`, `/storage/databases`, `reqwest ... TimedOut`: child 48.64s, wrapper 48.75s.
2. An assertion against the new observation API was added to the existing native owner-policy regression. It failed to compile with E0433/E0599 before implementation.
3. The first implemented native run failed after the test supplied a different trusted owner and then tried to read the original owner without completing the retained retirement fence. The test was corrected to assert that pending cleanup exists and explicitly complete original-owner-return cleanup before continuing the persisted-restriction checks. The fence was not weakened. Final native/runtime and complete Management suites pass.
4. Added public observation checks for exact database/filesystem selections and independent restriction flag, pending cleanup exact ID/old owner, stopped reopen, A->B->A, completed history versus pending history, fresh later cleanup ID, restrictive resource conflicts, and ambiguous resource bindings. Existing public policy conflict test also exercises the observation.
5. Extended the existing authenticated selections route regression with races after the initial identity read: final owner-active denial, learned revocation, and changed owner all refuse publication. Existing route ready selection/restriction and transfer tests remain.
6. Existing independent transfer tests cover stale ack, exact endpoint/cleanup-ID/current-owner binding, reopen, lost-response retry, and original-owner return. Production Create additionally executes the preserved reviewed restart/transfer scenarios.

## Actual results

Build durations are not scenario durations. Every nonzero result is listed, including compilation bounds and the initial zero-match filter.

| Check | Result |
| --- | --- |
| Native observation initial test | RED E0433/E0599 (API absent) |
| First implemented native run | FAIL, retained cleanup correctly denied original-owner read; 0.48s test, 2m13s build; test cleanup sequence corrected as above |
| Management server complete library | PASS twice: 38 passed / 2 explicit ignores, 12.99s then final 12.33s; final build 2m17s |
| Management selection policy (`replica`) | PASS 13 tests / 13.13s, then final expanded 14 tests / 13.87s; final build 1m18s |
| Independent transfer | PASS: 3 passed / 1 explicit wrapper-child ignore, 26.28s; actual HTTPS child 23.92s; completed retry build 1m35s |
| Production independent Create/reopen/reviewed restart cases | PASS child 73.44s / wrapper 73.54s; build 0.34s |
| Public policy cleanup HTTP chain | Initial wrong module filter matched ZERO tests, not validation; corrected filter PASS 1 / 27.47s |
| Native catalog response-time current-owner recheck | PASS 1 / 8.20s |
| Storage local replication authorizer | PASS 4 / 6.51s; build 2m39s |
| Storage Management client | PASS 10 / 0.08s |
| Storage delayed SQL-frame/filesystem-content release checks | PASS 2 / 40.21s |
| Strict affected-library Clippy | PASS `-D warnings`, 13.24s |
| Scoped rustfmt / `git diff --check` | PASS |
| Initial post-change source Join | FAIL same first catalog GET; child 46.98s / wrapper 47.09s |
| First final paired source/historical Join run | PASS both: source child 66.79s, historical child 66.78s, combined wrappers 133.83s |
| Repeat paired source/historical Join run | MIXED: source FAIL same first catalog GET, child 47.89s; historical PASS child 67.53s; combined wrappers 115.65s |

Two calls hit `timeout_ms:200000` during compilation: the initial post-change source Join build and the first transfer build. Both stopped in owner-library compilation, before any fixture result. Their later bounded commands completed. No scenario bound was increased. No timeout is reported as a passing test.

### Measured first-catalog path

Cumulative Management route milliseconds, measured from route entry:

| Run / observed route read | Authentication + identity | After reconciliation (old path) | After selection/observation | After FINAL recheck |
| --- | ---: | ---: | ---: | ---: |
| Baseline failing source (`01a1592c-4126-7608-a6fe-699f3a0af923`), first | 2759 | 2796 | 2883 | 3239 |
| Baseline, next | 1312 | 1350 | 1435 | 1791 |
| Repeat failing source (`01a15950-ca5c-72be-b83d-c50f65461a0b`), first | 2612 | — | 2663 | 2999 |
| Repeat failing source, next | 811 | — | 866 | 1627 |
| Repeat failing source, response-time read | 1298 | — | 1350 | 1687 (logged after client timeout) |

The old local reconciliation+selection intervals were 124ms and 123ms; the final failing run's observation intervals were 51ms, 55ms, and 52ms. This measures removal of duplicated local work, not a controlled whole-request performance guarantee. Remote authentication/identity and final current-owner checks still dominate. Phases overlap with existing background owner work; do not sum different route/request log entries as one request duration.

For the final failing source run, Storage service-token acquisition was 936ms and completed introspection 3149ms cumulative; response-time service-token was 0ms and introspection 511ms. The first catalog GET then failed at unchanged `independent_join_tests.rs:515` with `TimedOut`. The final Management recheck still executed. No later catalog request, warming, or retry was used to excuse this failure.

Passing source fixture: `01a15947-a4c5-70dd-a424-12e553a929aa`. Passing historical fixtures: `01a15948-aa36-7358-8f0e-3641dc74100b` and repeated `01a15951-85ed-7126-be75-1b2ce98866b0`. Passing Create fixture: `01a15952-edf5-737f-b82b-34464f50c257`. All use isolated native roots and mock trust/provider commands; these are not certification of real OS trust/provider installation.

## Commands and controls

Working directory `/home/nathan/code/rust/aicacia/of`. Sole granted Rust build token used sequentially; no concurrent Cargo builds. Rust analyzers PIDs 17703 and 23931 were paused before Cargo work and resumed on release; no editor config changed. Every Cargo call used `timeout_ms:200000`, `RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0`, `--offline --locked`, and these exact unchanged receipt-38 profiles:

```text
--config 'profile.dev.package.unified-server.debug=0'
--config 'profile.dev.package.unified-server.opt-level=0'
--config 'profile.dev.package.storage-server.debug=0'
--config 'profile.dev.package.idp-server.debug=0'
--config 'profile.dev.package.management-server.debug=0'
```

Each test invocation below is `cargo test --offline --locked` plus the listed arguments, the five profile flags above, then `-- --nocapture --test-threads=1`, with the exact environment above. Repeated test commands used identical controls; their outcomes are retained, not hidden by repeated attempts.

```text
-p unified-server --lib production_independent_join_and_source_offline_reopen
-p management-server --lib native_owner_policy_requires_binding_and_rechecks_persisted_restrictions
-p management-server --lib
-p management-service --features replica --lib replica::selection_policy::tests::
-p unified-server --lib independent_transfer_tests::
-p unified-server --lib production_independent_join
-p storage-server --lib local_replication_authorizer::
-p storage-server --lib management_client::tests::
-p storage-server --lib delayed_http_policy_cannot_release
-p unified-server --lib policy_cleanup_http_tests::
-p unified-server --lib native_catalog_paused_local_owner_rechecks_before_response_without_rollback
-p unified-server --lib policy_cleanup_tests::
-p unified-server --lib production_independent_create_and_offline_reopen
```

Clippy was `cargo clippy --offline --locked -p management-server -p management-service -p storage-server -p unified-server --lib`, with that environment and those five profile flags, then `-- -D warnings`.

Formatting was `rustfmt --edition 2024 --check` on the four own Rust files. `git --no-pager diff --check` also passed. No powerset, workspace sweep, real OS trust change, deadline increase, request warming, production retry, or policy cache.

### Captured output hashes

These hashes identify captured command output used to transcribe this receipt; temporary logs/snapshots were removed after transcription rather than left as extra source/evidence files.

| Output | SHA-256 |
| --- | --- |
| Baseline source Join | `08da5d018e71045822cef7395fc127b5bb3be57ed2628095f00a27409baa26b2` |
| Initial observation compile RED | `41e4bfa5bccdd95affc9371a0bd0c47f93268f33c631d24f0566a5d04b1af35d` |
| First implemented native failure | `a85914b82fa9c4716b8edb968a3d0fe50f1b5184da7061db6056e01beced9729` |
| Initial post-change source Join failure | `54e001751735760947f468b0a24881668275445590c61fded741d83a062e0da0` |
| Paired Join pass | `48d4454f0cd053d330cb074968742b3cf3182eda7186fc530ab1fe4dd34b00e2` |
| Paired Join mixed repeat | `b649e03bdad32fe19d2ed43e0f1602da097ee5fddbc030c47478ebe3efae9284` |
| Final Management library | `aa6c54fee8892c0aa57fb3dc35b1bd6f9a6cc167b940f8f0c1ee39cf1618da6f` |
| Final Management policy | `e73952a27a689a21f535b29a76ab8fa26180d08575cd7c94410458d65b133bc9` |
| Independent transfer | `e11886659e54f830df01386eac753059b68d5266a9c9d911056967b29c46e787` |
| Create/review fixture | `6348ff584684c5651c50028734e728cc4104c2ad7d5104303e39ae231196ebb8` |
| Public cleanup HTTP | `188b691be5afefe34ecc802b6e4406512817e0b0588a92a865c3b45c462fc7bf` |
| Native response-time check | `32d4207a5de4193b1744f3331c78b9e085ffca3e1943d53800e2b27ffd061097` |
| Storage authorizer | `ea5b5792b3d15de03c9a5b042627a60d118d0eb0413cd5c98533cd01e827d6f3` |
| Storage Management client | `4795059773e391d2f13512c9150670872c9e8ad26dee7558039ef1b835883e7e` |
| Storage delayed release | `d21e4c6836c37a92d3a4d33a5a1d3b6db2236543c49928426708f9a714f7b32c` |
| Strict Clippy | `dcb2525fda25f3c91c2fc20a268ebd2603724ac549ec036154b427c466b52e39` |

## Remaining gate and secondary handoff

The Management observation is implemented and focused correctness checks pass. **The latest source Join is red; no stable five-second catalog qualification or completion claim.** No latency workaround was added and no commit was made.

The conditional IdP work was not bundled into this Management change. Read-only confirmation: `crates/idp-service/src/replica/replica_signer_repo.rs`, `trusted_signers()`, calls `allows_authorization()` inside the signer/member candidate-pair loop with alternating signer/member rows (lines 112–124 at this source). A separate next task can batch signer IDs and member IDs by table, preserving per-pair outcomes so one unrelated conflicted pair does not deny every signer. This is a concrete next investigation/implementation target, not a proven measurement that IdP batching alone will meet the remaining deadline. The parent can allocate it separately.
