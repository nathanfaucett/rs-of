# Receipt 38 — Independent lifecycle retry after external compile repair

## Outcome and source

Resumed on MAIN working tree, branch `integration/ui-driven-node-bootstrap`, HEAD `9c4de8db1feb16d3377d2305d280f58283ac7493` (reviewed Management restart/original-owner return fixes). Final Rust-only dirty diff SHA-256 (`git --no-pager diff --binary -- '*.rs' | sha256sum`): `ed8e4fd72a289259baef470d9306fd789f9f986b440c0186a6360a1a97d5ad95`. External `ofdb` HEAD at final inspection: `238210c5b822cd8b5f14274ea63d941ef884f6a0`, tracked dirty diff SHA-256 `0827bcc5f65fd8d10ffc861744e076a4d6a53007d7d9d25285db057cf5cbe26b` (does not include its untracked `sharded_btree.rs`). All twelve existing dirty Rust integration files and all reviewed production/test hunks were preserved. The historical receipt 37 is unchanged. This receipt supersedes its **current blocker**, not its historical observations.

**External E0599 is cleared.** User changes to `ofdb` import `StreamExt` and `pin_mut`; this turn did not modify external repositories. Both initial and final unified test-target checks pass. However, **qualification remains incomplete and no integration-fixes commit was made**: actual independent Join consistently times out at the first authenticated Storage catalog request. This is a measured runtime latency failure, not an external compile blocker. Its underlying bottleneck is not proven to be caused by external changes.

No tickets closed; no generated clients/TS edited; no real OS trust/provider/profile changes, powersets, CRAP, broad workspace sweeps, or increased deadlines. All real HTTPS fixtures ran through wrappers with isolated native roots and mock providers. The child scenario deadline remains 90 seconds; the native verification client's HTTP request bound remains five seconds.

## Changes made in this retry

- Preserved reviewed Management restoration during unavailable readiness and A->B->A cleanup logic, plus the pre-existing completion-history/schema/replay fences and exact historical-reset validators.
- Fixed the new review fixture's startup race: HTTP 200 `/setup/status` can mean `unfinished`; it now waits for **body `ready:true`** before requesting the service token. Before the fix, both Create variants got an empty HTTP 503 from the not-yet-published IdP router. Both now pass without weaker assertions or deadline changes.
- Removed the review fixture's unused `DeviceRepo` import.
- Resolved strict Clippy `collapsible_if` in Management cleanup-ID/conflict checking with an equivalent let-chain. No checks or failure ordering were removed.
- Added nonsecret Storage authentication phase timings (service-token and completed introspection) to the existing logger capture. No tokens, claims, secrets, credentials or subjects are logged.
- Improved the service-token fixture's failure diagnostic to display only non-success OAuth status/body; success tokens are not printed.
- Corrected import grouping and scoped formatting.
- A trial concurrent caller/inspected-token validation in the IdP introspection route did **not** fix Join's latency. It was removed; `crates/idp-server/src/router/routes/oauth2/introspect.rs` has **no final diff**. Both pre-access and response-time Storage authorization remain intact.

## Controls

Working directory for all commands: `/home/nathan/code/rust/aicacia/of`. Every Cargo tool call used `timeout_ms:200000`, offline/locked, one job, no incremental, 16 MiB Rust stack and receipt-36 selective profiles. Commands below are literal; repeat counts are noted in results. Compilation retries used identical commands/bounds.

Editor rust-analyzers PIDs 17703 and 23931 were paused with `kill -STOP 17703 23931` before the first Cargo call to prevent overlapping automatic workspace checks. No editor/project config changed. They are resumed on token release.

TDD skill was loaded at the already-agreed API/CLI seams. The observed red/green slice is the review fixture's startup/readiness race: repeated Create failures before waiting for actual readiness, followed by passing complete production fixtures. Join remains red and is not reported as accepted.

## Results

| Check                                                 | Actual result                                                                                                                                                           |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Unified `--tests` check                               | Initial PASS 30.92s (review unused-import warning); final PASS 18.22s, no warnings                                                                                      |
| Management/Storage startup/default ports              | PASS: 5 + 4 tests, 0.08/0.09s; unified filter matches zero tests                                                                                                        |
| Independent transfer                                  | PASS twice: 3 passed / 1 wrapper child ignored; suites 29.52s then 28.52s; actual HTTPS children 26.34s then 26.03s                                                     |
| Production independent Create                         | After race fix PASS 78.69s child / 78.81s wrapper; final measured run PASS 75.92s child / 76.04s wrapper                                                                |
| Production independent historical-record Create/reset | PASS 80.51s child / 80.61s wrapper; includes ordinary historical-startup refusal, explicit cleanup-only reset, provider/trust/data failure ordering and no regeneration |
| All-owner reset CLI explicit trust flag               | PASS: 1 test each IdP/Management/Storage                                                                                                                                |
| All-owner reset live setup-only exclusion             | PASS: 1 test each, 0.02s each                                                                                                                                           |
| Complete API library                                  | PASS: 35 passed / 2 explicit ignores, 0.88s, injected child checks also pass                                                                                            |
| Management selection policy (`replica` feature)       | PASS: 12 tests, 12.56s                                                                                                                                                  |
| Management OpenAPI inventory                          | PASS: 1 passed / 1 explicit exporter ignored, 0.05s                                                                                                                     |
| Storage local replication authorizer                  | PASS: 4 tests, 6.66s                                                                                                                                                    |
| Storage Management client                             | PASS: 10 tests, 0.08s                                                                                                                                                   |
| Storage delayed HTTP policy/content release           | PASS: SQL frame and filesystem queued-content regressions, 2 tests, 41.85s                                                                                              |
| Native `lidp --tests` check                           | PASS 28.06s                                                                                                                                                             |
| Native focused reset                                  | PASS: 6 tests, 0.23s                                                                                                                                                    |
| Strict affected-library Clippy                        | First failed at Management `collapsible_if`; after fix PASS 14.18s; final PASS 0.44s                                                                                    |
| Scoped fmt / diff / server dependency boundary        | PASS                                                                                                                                                                    |
| Independent Join/current and historical reset variant | FAIL before reset: first normal Storage GET exceeds unchanged five-second client request bound                                                                          |

A first Management-service policy command without `--features replica` matched **zero** tests (21.43s build). It is not counted as policy validation; the corrected feature command ran the 12 tests above. That zero-test invocation exposed inherited unused imports in `idp-service/src/oauth2/{authorization,scope}.rs` and dead `from_engine` in external `ofdb-sql`; no unrelated warning cleanup was attempted. Strict Clippy with the actual affected-library feature composition passes.

## Failure and timeout chronology

- Startup command: first three calls timed out during compilation at 200000ms (storage, unified test, Management test); fourth completed build in 1m13s and ran all nine startup tests successfully. No scenario deadline changed.
- Initial combined `production_independent` command completed in 183.09s with **four failures**: both Create variants at `storage_policy_token` HTTP 503; both Join variants at `independent_join_tests.rs:515` with `reqwest::Error { kind: Request, ... source: TimedOut }` for `/storage/databases`.
- Two further single Create attempts reproduced the race, child durations 41.35s and 39.82s, before the fixture readiness fix. OAuth error-body instrumentation confirmed empty 503, consistent with the setup fallback before router publication. Fixed Create subsequently passed, as above.
- Single Join retries failed at the same catalog request: child 49.17s (wrapper 49.32s), diagnostic run child 49.17s (wrapper 49.30s), trial concurrent-IdP-validation run child 48.09s (wrapper 48.23s). Trial removed, not retained as an alleged fix. These are HTTP request failures, not 90-second scenario timeouts.
- Reset CLI: first three calls timed out compiling storage/IdP/Management tests; fourth completed build in 1m14s and all three parser tests passed.
- Historical Create/reset: first call timed out compiling; identical retry completed and passed (80.51s child).
- Management policy with `replica`: two compilation timeouts; third completed build in 2m22s and passed all twelve tests.
- Native reset: two compilation timeouts; third build 7.02s, six tests passed.
- Management OpenAPI: one compilation timeout; retry build 1m13s, inventory passed.
- Storage local-authorizer: first call spent 3m18s building and was killed by the 200000ms tool bound after the first test started (no pass claimed); identical retry build 0.33s, all four passed.

## Actual independent phase observations

These are real fixture `Instant` messages, not acceptance latency, build time or substitute unified measurements. Phases overlap across owners; do not sum them into a scenario duration. Mock-provider and isolated trust costs are not production OS/provider installation certification.

### Passing final Create fixture

Source: `independent-create-01a1591f-13d8-7722-9556-5c762a46afb5`, ordinary Create wrapper, child 75.92s including interruptions, recovery, policy negatives, reviewed restart cases, offline reopens and reset checks.

| Owner      | Phase                                                        | Elapsed ms |
| ---------- | ------------------------------------------------------------ | ---------: |
| Management | consumer/endpoint keys, interrupted attempt / retained retry |    20 / 11 |
| IdP        | consumer key                                                 |          3 |
| Storage    | consumer keys                                                |          6 |
| IdP        | schema/open                                                  |       1770 |
| IdP        | protected inventory                                          |        211 |
| IdP        | password credential                                          |        433 |
| IdP        | signing-key/root seed                                        |        817 |
| IdP        | Client registration seed                                     |       1525 |
| IdP        | serving                                                      |         53 |
| Management | schema/open                                                  |        178 |
| IdP        | relationship registration/wait                               |       2472 |
| Storage    | relationship wait                                            |       7508 |
| Storage    | catalog open/serving                                         |          7 |
| Management | semantic seed                                                |       3494 |
| Management | serving                                                      |          3 |

Management current-policy cumulative phases on the first measured read: authentication/identity 1622ms, reconcile 1644ms, selection 1688ms, final recheck 1870ms. These are cumulative from route entry, not independent phase costs.

### Failing Join diagnostic fixture (not accepted)

Source: `independent-join-01a158e7-cc66-7402-a1de-5a22bc7bb6ec`, diagnostic run before the subsequently removed concurrent-validation experiment.

| Owner      | Phase                                           | Elapsed ms |
| ---------- | ----------------------------------------------- | ---------: |
| IdP        | initial consumer key / retained retry           |      4 / 0 |
| Management | initial consumer/endpoint keys / retained retry |     13 / 8 |
| Storage    | initial consumer keys / retained retry          |      7 / 0 |
| Management | initial source discovery / retained retry       |    321 / 0 |
| IdP        | initial schema/open / retained retry            | 1883 / 722 |
| IdP        | initial protected signer / retained retry       |    249 / 0 |
| Management | approval wait                                   |       5135 |
| IdP        | approval wait                                   |       4312 |
| Management | schema/open after approval                      |        166 |
| Management | approved public import                          |       2253 |
| IdP        | approved public import                          |       7755 |
| IdP        | local Client seed                               |       2421 |
| IdP        | serving                                         |        123 |
| Management | serving                                         |          7 |
| IdP        | relationship registration/wait                  |       4519 |
| Storage    | relationship wait                               |      20076 |
| Storage    | catalog open/serving                            |          4 |

The first Storage request logged service-token 971ms and completed IdP introspection 3240ms cumulative. Its response-time pass logged cached service-token 0ms and introspection 514ms. Management's first relevant policy recheck completed at 1632ms cumulative; the next response-time read logged authentication/identity 1313ms, reconcile 1350ms, selection 1436ms but did not finish its final recheck before the five-second outer HTTP deadline. Both required authorization passes remain. This narrows the latency failure to serial remote current-evidence work across pre-access and pre-response checks; it does not isolate which owner/persistence operation should be changed. No cache, warming request, removed recheck, artificial retry or timeout increase was introduced to manufacture a pass.

Build/profile costs are reported separately above. Trust installation/removal used wrapper stubs, not real host trust. Mock protected-provider key operations are included only in their labelled fixture phases.

## Literal Cargo commands

### Unified checks (initial and final)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo check --offline --locked -p unified-server --tests --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0'
```

### Startup/default ports (four identical attempts)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-server -p storage-server -p unified-server --lib startup::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Actual independent transfer (two runs)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib independent_transfer_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Initial four-fixture lifecycle run (failed)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Create (two failed reproductions, then two passing runs)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent_create_and_offline_reopen --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Historical-record Create/reset (two attempts)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent_create_legacy_record_reset --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Join (three additional failing retries)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent_join_and_source_offline_reopen --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### All-owner reset parser (four attempts)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-server -p management-server -p storage-server --lib local_reset_requires_explicit_os_trust_removal --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### All-owner reset live lock

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-server -p management-server -p storage-server --lib reset_cannot_remove_a_running_setup_only --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Strict affected-library Clippy (lint failure, fixed pass, final pass)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo clippy --offline --locked -p api -p idp-server -p management-server -p management-service -p storage-server -p unified-server --lib --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- -D warnings
```

### Native check

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo check --offline --locked -p lidp --tests --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0'
```

### Native reset (three attempts)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p lidp --lib setup_reset::tests --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Management policy (first zero-match command; corrected feature command three times)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-service --lib replica::selection_policy::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-service --features replica --lib replica::selection_policy::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Management OpenAPI (two attempts)

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-server --lib router::openapi_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### API

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p api --lib --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Storage policy (two attempts), client and delayed-response/frame checks

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p storage-server --lib local_replication_authorizer:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p storage-server --lib management_client::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p storage-server --lib delayed_http_policy_cannot_release --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

## Static/source boundary

Scoped rustfmt was applied, then checked on the fourteen dirty Rust files. `git --no-pager diff --check` and `python3 scripts/check_server_dependencies.py` passed; the latter reports `No direct server-to-server dependency edges found.` The review receipt move remains preserved and pending tracking under `.scratch/ui-driven-node-bootstrap/evidence/review-transfer-lifecycle.md`; its old tracked location is deleted. This turn does not treat that pre-existing move as data to discard.

## Remaining acceptance gate

Resolve the cold independent Join catalog latency without removing pre-access or pre-response current-policy checks, enlarging request/scenario bounds, adding warming/qualification to production setup, or disguising it as an external compiler failure. Then rerun both current/historical Join/reset wrappers, final focused affected-source checks and any scope expanded by the root-cause fix. The historical Join cleanup variant has not reached its reset assertions. Review and commit the coherent dirty integration fixes only once critical qualification passes. SDK/tooling and ticket work remain separate; nothing here closes tickets or certifies actual OS trust/provider installation.
