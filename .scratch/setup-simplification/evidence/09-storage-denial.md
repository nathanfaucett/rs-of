# Ticket09 Storage denial slice

**Current status:** identity-first denial and fixture prerequisite fixes retain their recorded validation. The original 2MiB database replication regression now has **current red -> green** with the owner-local schema execution-stage fix committed on ofdb branch `implement/setup-engine-cost` as `7aeb114`; primary ofdb is restored and awaits parent review/merge. The prior unresolved-stack sections and failed outer-boxing experiments remain historical evidence, not current closure claims. Full-suite qualification, the previously recorded full unit-test Clippy diagnostics, and shared ticket09 remain separate gates.

## Scope and inputs

- Worktree: `of/.worktrees/setup-09-storage`; branch: `implement/setup-09-storage`.
- Confirmed clean entry HEAD/base: `aec614b32` (`integration/setup-simplification-followup` at entry).
- Loaded TDD and ponytail; read local AGENTS, setup spec, ticket09, shared timeout evidence, shared/Storage glossaries and Storage security contract.
- Ownership: relevant `crates/storage-server` source/tests and this new evidence file only. Shared ticket09/evidence/map/spec and sibling repositories are not owned or edited.
- Complexity: low implementation complexity, high authorization/cancellation risk, focused ticket09 priority. No new dependencies, layers, bounds, retries, sleeps or token/policy caches proposed.

## Source mechanism and proposed semantics

`IdpClient::introspect` currently joins identity inspection and `local_owner`, then propagates the identity result. Therefore even a definite identity rejection waits for policy completion. The same public client is used by Storage resource authorization and socket admission. Resource handlers separately call authorization again before returning successful resource responses.

`LocalReplicationAuthorizer::from_callbacks` serializes local policy work, including pending owner-copy removal, exact-binding readback, acknowledgement and post-acknowledgement readback. Replacing `join!` with cancellation-on-error concurrency could drop that work mid-flight. The minimal candidate is identity-first: propagate identity failure before starting policy; on identity success, await the full current policy lookup and compare the subject to the current owner. This avoids introducing a new identity-error-driven cancellation point inside policy cleanup. It does not make externally cancelled requests cancellation-safe, change the cleanup implementation, or guarantee cleanup on invalid requests.

Tradeoff: successful identity and policy work no longer overlap, so valid-request latency can be their sum instead of their maximum. Every valid identity must still consult current policy, with no early rejection shortcut, historical policy reuse or response-recheck removal. Failed identity preserves its original `InvalidToken`/`ServiceUnavailable` distinction, mapped by Storage HTTP to 401/503 respectively.

## Deterministic regression and execution gate

Authorized seams: public `IdpClient::introspect` and the existing `resource_router` HTTP request seam, using public local identity/policy callbacks. Prepared `crates/storage-server/tests/identity_policy.rs::identity_failure_returns_without_waiting_for_device_policy` before any production change. Initially placed in the existing resources unit-test module, then moved to a focused integration target after two RAM stops compiling the full library test harness. The original source module is restored exactly; the integration test uses only public APIs.

The identity callback returns an immediate definite error. Policy resolution waits on a zero-permit semaphore. The test polls each real public operation once, records whether it completed while policy remained gated, then releases/drains any pending operation before asserting. No elapsed-time threshold, sleep, retry, retained native handle or production instrumentation is involved. Both direct introspection and the actual Storage route are exercised before the readiness assertion. Invalid-token and identity-service-unavailable cases retain distinct expected results.

The Rust slot was explicitly granted after agent09's RAM stop. Initial preflight measured about 2.8GiB MemAvailable, memory PSI near zero, full swap, and ample disk. Full swap was not used as a veto. Used the existing `/tmp/setup09-default-monitor.py`, shared root target `/home/nathan/code/rust/aicacia/of/target`, jobs=1, unchanged profiles, one-second resource samples, immediate stop below 2,097,152KiB MemAvailable, sustained memory PSI avg10 some>=20/full>=10 for 20 seconds, and 550-second stage ceiling. Only each spawned process's own new-session PGID was terminated.

Build attempts (cwd this worktree):

```sh
# Attempts storage-red-build1 and storage-red-build2:
cargo test --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p storage-server --no-default-features --features network --lib identity_failure_returns_without_waiting_for_device_policy --no-run --message-format=json
# Attempt storage-red-integration-build1:
cargo test --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p storage-server --no-default-features --features network --test identity_policy --no-run --message-format=json
```

| Attempt                          | Actual outcome                                         | Seconds | Minimum available KiB | Peak sampled owned RSS KiB | Max memory PSI avg10 some/full |
| -------------------------------- | ------------------------------------------------------ | ------: | --------------------: | -------------------------: | ------------------------------ |
| `storage-red-build1`             | RAM stop during Storage compilation                    | 415.189 |             2,068,036 |                  1,449,444 | 1.41 / 1.29                    |
| `storage-red-build2`             | RAM stop during Storage test-harness compilation       |  18.659 |             2,082,024 |                  1,467,184 | 0.18 / 0.18                    |
| `storage-red-integration-build1` | RAM stop during production Storage library compilation |  71.466 |             2,022,208 |                  1,159,996 | 0.42 / 0.36                    |

The second attempt followed verified group exit and recovery to roughly 3GiB available. The third changed only the test target to avoid unrelated unit-test compilation, not any production behavior, security check, feature, profile or guard. It still could not finish compiling the production library. Each returned wrapper exit143/Cargo SIGTERM, not a test failure. Saved receipts, logs and resource JSONL use `/tmp/setup09-qual-<attempt>{.json,.log,-samples.jsonl}`. Log SHA-256 respectively: `942b5b910427461eb0ef1bdf819b9872b2e4e97ec9e0b73e70596e159e40da5d`, `6ce8d47cc329b53bb869a52d08c004620bc869151771e3e7abe6d03c7226e22b`, `0af34bd604a868d8edce24da7edb4298addd37572d8aae4d253780883d998b52`.

Verified all three owned PGIDs (`3368270`, `3375287`, `3380446`) absent after stopping. No unrelated process was killed, no caches deleted, and no sibling/profile changes made. Final preflight still showed only about 2.75GiB available, with unrelated tooling RSS approximately 5.4GiB (`rust-analyzer` PID2965212), 2.35GiB (`rust-analyzer` PID3062670), and 5.95GiB (Zed PID2785858). These are memory-relief clues for the parent, not permission to terminate those processes or proof of a historical timeout cause.

**No test has executed; no actual red or green exists yet.** Production remains unchanged. Further compilation is paused pending external headroom improvement rather than repeatedly hitting the same guard. Actual red is required before applying the candidate fix; green and valid/current-policy/response-recheck controls remain pending. Matrix, Clippy and CRAP have not run. No validated behavioral change or closure is claimed. Parent subsequently requested preserving the unexecuted regression and evidence in a clearly named WIP commit, with hooks disabled, then merging integration `5359c17fa` into this implementation branch only. No further build is authorized under current low headroom; no production fix is authorized in this WIP handoff.

## WIP handoff and exact resumption

Only the new integration test and this evidence file belong to the Storage WIP commit. Existing `resources.rs` is unchanged; there are no dependency, lockfile, feature-definition, profile or production-code edits. Rustfmt check on the test and `git diff --check` passed. This is source/format validation only: compilation, actual regression red, green, valid-current-policy controls, response-recheck controls, Clippy, feature matrix and CRAP are **NOT RUN to completion**. Keep this worktree for resumption. Integration `5359c17fa` brings other agents' committed diagnostics; merging it is not fresh validation of those diagnostics or ownership of their files. Do not merge this WIP into the root/integration branch.

After explicit resumption authorization, improved measured headroom and exclusive Rust slot, the exact next red command (cwd `/home/nathan/code/rust/aicacia/of/.worktrees/setup-09-storage`) is:

```sh
python3 /tmp/setup09-default-monitor.py storage-red-integration-run1 cargo test --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p storage-server --no-default-features --features network --test identity_policy identity_failure_returns_without_waiting_for_device_policy -- --exact --nocapture
```

The monitor file and its receipt prefix are local `/tmp` artifacts, not committed tooling. At WIP handoff the monitor exists and `storage-red-integration-run1` log/receipt paths are unused. Reconfirm the monitor remains present, its RAM/PSI guards are unchanged, the receipt prefix is unused, and resource headroom/owned-group cleanup before execution. The command compiles and runs the one regression; a build/resource stop must not be reported as actual red. Do not apply the identity-first candidate until the observed test failure establishes the public mechanism. Green plus valid-identity/current-policy and response-recheck controls remain required afterward.

**Feature prerequisites:** the focused Rust test requires `network`; without it the entire integration test is cfg-disabled and a zero-test run proves nothing. It requires neither default CLI/Swagger features nor Node/JavaScript SDK builds. The existing shared target/cache and worktree sibling dependency paths resolved in the recorded builds; profiles and dependencies remain unchanged.

**SDK prerequisites for broader owner-boundary/full-suite validation (not for this red test):** read-only file checks in this worktree found no `node_modules`, no built `packages/{idp,management,device}-client/esm/index.js`, an existing `packages/storage-client/esm/index.js`, and no `packages/native-fetch/esm/index.js`. File presence is not runtime qualification. The actual owner-boundary SDK scenario additionally requires `packages/storage-client/esm/generated/index.js`, which is also absent in this worktree. Prepare this worktree with `pnpm install --frozen-lockfile --ignore-scripts && pnpm sdk:build` before broader Cargo tests that exercise the exported SDK, then validate all five exports. Native-fetch ESM is a separate prerequisite for SDK/OIDC Vitest suites, not this Rust regression. No Node/install/SDK build was run for this WIP. Parent will request permission for a Rust-analyzer restart rather than silently killing user tools.

## Attribution boundary

This demonstrated mechanism does not establish the cause of every historical Storage timeout. Archived failing requests show overlapping dependent token work, but do not identify the exact completion time of identity rejection versus policy completion. Authenticated Storage timeouts, remote readiness, permission requests and Join history/readiness failures remain separate questions. No full-suite reliability, historical common cause or ticket09 closure is claimed.

## Authorized resumption and implementation, 2026-10-26

### Authorization, fixed inputs and scope

Parent reported explicit user authorization to restart the two identified Rust analyzers, verified their identity and SIGTERMed them, then granted this agent the exclusive Rust slot. This agent did not terminate user tooling. Re-measured **11,090MiB MemAvailable**, memory PSI avg10 some/full **0/0**. Used the same jobs=1 shared target, monitor, RAM floor, PSI guard, 550-second stage ceiling and `RUST_MIN_STACK=67108864`; monitor unsets `RUST_LOG` and `OF_SETUP_TIMEOUT_TIMING`. No swap-only gate, profile change or simultaneous agent build.

Resumed clean WIP merge HEAD `b06d319d2a8f3580ae28692e3cfbe11a6fc236dd`, then merged requested integration `1cfe76076d2441b343546cf966b46972b2f1ca23` before execution, producing pre-fix HEAD **`420b7dad97cb5e6ac363c34757e1a61e5b560808`**. Re-loaded TDD/ponytail and read root/new app AGENTS; no app work is part of this slice. Entry/WIP commits remain `6a4b1297e` and `b06d319d2`. Cargo.lock SHA-256 **`bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`**. Read-only sibling checks: clean `ofdb` **`e3242170c94d5fc4bcc121db812310c8106c7ab6`**, clean `ofnet` **`dbea925a0a3f99053fffc02618baccc2a1cbea9f`**, clean `offs` **`df4c696a910296635473ec296d9235acbaa60d9b`**. No sibling writes, SDK/Node build, shared ticket/evidence/map/spec edit, dependency, lockfile, feature or profile change.

### Actual red before production change

Ran the exact guarded red command recorded above, prefix **`storage-red-integration-run1`**. Compilation completed; the actual test ran and failed with exit101:

```text
running 1 test
definite InvalidToken must not wait for gated policy: introspection_ready=false, http_ready=false
0 passed; 1 failed; 0 ignored; 0 filtered out
```

Both actual public operations had completed identity rejection but remained pending behind gated policy. The test released/drained policy work before asserting, so it did not create a retained handle or cancel an in-flight cleanup future to manufacture red. Supervised elapsed **178.785s** including compilation, test time **0.00s**, no resource stop. The first invalid-token assertion ends the red test before its service-unavailable iteration; both iterations execute in green.

### Minimal production fix and precise semantics

Changed only `IdpClient::introspect` production logic in `crates/storage-server/src/idp_client.rs`: await identity inspection and propagate its error first; only successful identity starts/awaits `local_owner`, then compare the subject with the current owner. The no-policy path returns the same identity result. Original error mapping remains: identity `InvalidToken` ->401, identity `ServiceUnavailable` ->503; local-policy lookup/validation failure or wrong owner ->`InvalidToken`/401.

Chose identity-first rather than a fallible join/select: `local_owner` can serialize owner-copy removal, exact cleanup readback and acknowledgement. Dropping a concurrent policy future on identity error would introduce an avoidable cleanup cancellation point. Detaching/draining it in a spawned task would add lifetime/shutdown ownership rather than be a minimal fix. Identity-first starts no request-local cleanup for rejected identity, and awaits the existing whole policy operation on valid identity. It neither cancels other requests' policy work nor strengthens externally cancelled requests' cleanup guarantees. No cached policy, early valid-request rejection, detached task, layer, retry, sleep or deadline change.

**Performance tradeoff:** successful identity and policy no longer overlap in this method; their serial latency can be the sum instead of the maximum. This change removes the unnecessary policy dependency for definite identity errors, not all authentication cost, queue contention or authenticated-request timeouts. Every valid identity still requires current approved/active/admin-allowed exact-endpoint owner policy, and successful resource responses still reauthorize. No latency benchmark or broader reliability improvement is inferred from the deterministic test.

### Actual green and public controls

Immediately reran the same regression command with prefix **`storage-green-integration-run1`**, after only the production fix: **1 passed**, exit0, **79.663s** including compilation, test time0.00s. Both error iterations passed: prompt rejection while policy remains gated, distinct 401/503 behavior.

Expanded only the owned integration test, then ran prefix **`storage-controls-integration-run1`**, **4 passed**, no failed/ignored/filtered tests, **5.089s** including compilation. Tests run through public `IdpClient` and existing `resource_router`/Tower HTTP request seams, not private helpers or a new fake framework:

- `identity_failure_returns_without_waiting_for_device_policy`: immediate definite invalid identity and identity-service failure both complete without awaiting gated policy; errors and Storage HTTP status mapping preserved.
- `valid_identity_waits_for_current_policy_and_observes_learned_denials`: valid identity remains pending until current policy is released, accepts its approved owner, then observes changed owner, revocation, inactive owner, admin restriction, mismatched endpoint, nil Device ID and unavailable policy on repeated direct/HTTP requests. Restored current policy accepts again; no historical-policy reuse or denial of valid identity to avoid policy work.
- `missing_bearer_is_denied_without_identity_or_policy_work`: actual Storage route returns401; supplied callbacks panic if invoked.
- `storage_response_rechecks_policy_before_returning_success`: real isolated `DatabaseRuntime` catalog plus actual Storage database-list router first returns200/`[]` with valid identity/current policy. On the next request, gate its response-policy lookup, learn local revocation, release and observe401 with empty body rather than successful resource disclosure. The gate is at the fourth injected lookup (response check of the second request), not an assertion that production call counts are themselves the contract. No Storage handler or response recheck was modified.

### Scoped validation commands and outcomes

All Cargo commands below were prefixed by `python3 /tmp/setup09-default-monitor.py <prefix>` and used this common prefix:

```sh
cargo <test-or-clippy> --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p storage-server --no-default-features --features network
```

| Receipt prefix                      | Remaining command arguments                                       | Actual result                                                               | Supervised seconds |
| ----------------------------------- | ----------------------------------------------------------------- | --------------------------------------------------------------------------- | -----------------: |
| `storage-controls-integration-run1` | `test`: `--test identity_policy -- --nocapture`                   | 4 passed                                                                    |              5.089 |
| `storage-idp-library-run1`          | `test`: `--lib idp_client::tests -- --nocapture`                  | 9 passed /79 filtered                                                       |            186.326 |
| `storage-resources-library-run1`    | `test`: `--lib router::resources::tests -- --nocapture`           | 6 passed /82 filtered                                                       |              0.549 |
| `storage-policy-library-run1`       | `test`: `--lib local_replication_authorizer:: -- --nocapture`     | 5 passed /83 filtered, including real cleanup/binding/cancellation controls |              4.690 |
| `storage-clippy-run1`               | `clippy`: `--lib --test identity_policy --no-deps -- -D warnings` | passed, exit0                                                               |             74.703 |
| `storage-library-all-run1`          | `test`: `--lib` (default test concurrency/capture retained)       | **failed/aborted**, exit101, no final aggregate                             |             15.284 |

Existing scoped HTTP tests also exercise real IdP HTTP client mapping of revoked tokens/outage, malformed principal/audience, missing bearer and read-only write denial. Scoped library groups passing is separate from the full-library failure.

**Broader failure retained, not retried or reclassified:** the actual all-library run announced88 tests, marked these two existing tests FAILED:

- `filesystem_transport::execution_authorization_tests::admitted_socket_write_finishes_then_next_write_observes_local_denial`
- `filesystem_transport::execution_authorization_tests::queued_response_observes_local_grant_revocation_before_authenticated_socket_write`

It then aborted with `thread 'tokio-rt-worker' ... has overflowed its stack`, `fatal runtime error: stack overflow, aborting`, binary **SIGABRT**, Cargo exit101. Libtest capture and process abort mean the individual assertion bodies were not printed and there is no completed aggregate result; no invented assertion diagnosis or final pass count is supplied. This was not a RAM/PSI stop. Those execution fixtures use their own `FrameAuthorizer` and real owner repositories rather than the changed `IdpClient` seam, but no pre-fix baseline execution establishes their historical cause or absolves the branch. Did not rerun serially/raise stacks/retry-to-green, change unrelated fixtures, or claim full-library success.

`rustfmt --edition 2024 --check crates/storage-server/src/idp_client.rs crates/storage-server/tests/identity_policy.rs` and `git diff --check` passed. Feature-powerset/full-workspace matrix, `just crap`, coverage, default CLI/Swagger builds, SDK prerequisite installation and broader owner-boundary HTTPS qualification are **NOT RUN** in this resumed slice. The broad Storage-lib abort and documented missing SDK prerequisites remain explicit parent validation gates rather than claims of completion.

### Resource receipts, artifact identity and handoff

All resumed stages completed without a resource/stage guard stop; all sampled memory PSI avg10 some/full were **0/0**. Across resumed stages minimum available RAM was **9,063,376KiB**; peak sampled owned group RSS **3,552,368KiB**; highest reported kernel individual-child RSS **3,255,696KiB**. Short subsecond stages can finish between one-second samples; sampled RSS is not an exact runtime peak. Verified every recorded Storage-owned PGID absent after stages, including the aborted full-library process. No unrelated kill, cache deletion, instrumentation or profile changes.

Receipts/logs/samples: `/tmp/setup09-qual-<prefix>{.json,.log,-samples.jsonl}`. Cargo red/green and final integration binary is `target/debug/deps/identity_policy-e4bfab3fe9467f8a`; library binary is `target/debug/deps/storage_server-eeafeffb4be7bf1e`. Red and green rebuild the same binary path; saved stage logs/hashes identify the observed outcomes, not a claim that the red executable remains retained.

Log SHA-256:

| Prefix                              | SHA-256                                                            |
| ----------------------------------- | ------------------------------------------------------------------ |
| `storage-red-integration-run1`      | `cfdd93556536896ede4db6e07e9cfa9f18a48c735e0ade074c5bdf6191224bbb` |
| `storage-green-integration-run1`    | `6d58c7a043b6baf578d393ce8c4d26d5ec147bec0fe9ba7256879ab4f6152eb1` |
| `storage-controls-integration-run1` | `e7c53ebb3b85964ae225cce77e7438a904b4439e24183a84b4aea107355722a7` |
| `storage-idp-library-run1`          | `01907bf851978d9c3df6de8ee57425170345ff84ce82734276057374ca979371` |
| `storage-resources-library-run1`    | `f94215db8e2e8341a648c496f58d14f59da9c1435b806778ae6c0dc189751420` |
| `storage-policy-library-run1`       | `5b4ac1a92995c4a1f25f175bf62e107e3da700272241f86dc2850947ee9dd3ba` |
| `storage-clippy-run1`               | `9d35256132b24aa99a57903ff2d6b446215a05c8fa03f10f65f3caddd47655cc` |
| `storage-library-all-run1`          | `321e51a05c12e284df60760c9e2517eaa86785173baf32106268bacdaa83825d` |

Implementation disposition: commit the focused production fix, expanded public tests and this evidence with hooks disabled, merge current integration tip into this branch before handoff, retain the worktree and release the Rust slot. Do not merge into root/integration or close shared ticket09. This delivers the demonstrated Storage denial-dependency fix with validation limits, not historical common-cause attribution or full setup qualification.

## Validation-follow-up: missing fixture policy schema and isolated stack failure

Parent authorized the specific fixture follow-up and exclusive Rust slot after history diagnostics completed. Fast-forwarded clean `366afcd6e` to requested integration **`bcebfb0fa7aaed5af200846c86f6b6ed576651a7`** before execution. Re-measured about11,055MiB available, memory PSI0/0, full swap. Same supervisor, shared target, jobs1, network-only features, optimized+debuginfo profile and 64MiB environment minimum; no guard/profile/dependency changes. Source/log diagnosis while another agent owned the preceding slot ran no builds/tests and made no edits.

### Fixture red and minimal prerequisite repair

Ran each existing test **individually**, with the common Cargo prefix above and `--lib <fully-qualified-name> -- --exact --nocapture`:

- `filesystem_transport::execution_authorization_tests::admitted_socket_write_finishes_then_next_write_observes_local_denial`, receipt **`storage-fixture-red-admitted1`**, exit101,3.060s supervised/2.38s test.
- `filesystem_transport::execution_authorization_tests::queued_response_observes_local_grant_revocation_before_authenticated_socket_write`, receipt **`storage-fixture-red-queued1`**, exit101,2.704s supervised/2.26s test.

**Both recovered the same actual panic at `execution_authorization_tests.rs:74:224`:** `explicit actor grant: InvalidInput("Invalid query: Table not found")`. This supplies the assertion evidence lost when the previous full-library process aborted. Neither was a resource stop or a policy/write assertion failure.

The shared fixture opens unique UUID-scoped IdP and Management redb files. It initialized all IdP migrations in the IdP file, but only Device migrations in the Management file, then constructed Management role/permission repositories against the latter. Device migrations intentionally omit policy tables. Provisioning queries `roles` before creating the explicit grant; Management's separate `replica::up` aliases `idp_model::replica::up_policy`. No shared-root/global-backend race is established or needed to explain this missing table.

Minimal fixture-only change in `execution_authorization_tests.rs`: initialize Management policy schema in that same fixture Management file after Device schema initialization; correct the Device schema expect label. No grant, actor/action/target, queue, current-policy, in-flight completion, persisted-denial, timeout or production behavior was changed.

**Green:** receipt **`storage-fixture-green1`**, common Cargo prefix plus `--lib filesystem_transport::execution_authorization_tests:: -- --nocapture`; **2 passed /86 filtered**, exit0,130.129s supervised including compilation/4.08s test. The two tests ran together under default libtest concurrency; their original negative impersonation/target assertions, revocation-before-write, admitted-write completion, next-write denial and offline persisted-denial checks all completed. This prerequisite repair is committed separately from the identity-first denial production fix.

### Three unchanged 2MiB worker tests, each isolated once

Same common Cargo prefix, `--lib replication_runtime::tests::<name> -- --exact --nocapture`; no stack/bounds change:

| Receipt prefix                | Test name                                                                     | Actual result                                                    | Supervised seconds |
| ----------------------------- | ----------------------------------------------------------------------------- | ---------------------------------------------------------------- | -----------------: |
| `storage-stack-database1`     | `database_changes_from_higher_id_and_new_peers_catch_up_then_stay_idle`       | **worker stack overflow, SIGABRT, Cargo101**, no final aggregate |              2.025 |
| `storage-stack-responder1`    | `responder_only_peer_removal_cancels_persistent_filesystem_session`           | 1 passed /87 filtered                                            |              0.897 |
| `storage-stack-invalidation1` | `invalidation_and_feed_closure_fence_sessions_while_configuration_is_blocked` | 1 passed /87 filtered                                            |              0.694 |

The database scenario is now a concrete isolated current-code stack failure, not inferred from completion order. Source `on_two_mib_workers` explicitly sets two workers at2MiB, overriding the supervisor's `RUST_MIN_STACK=67108864` request for those threads. These unchanged files also exist at pre-denial integration; source identity alone still is not a runtime baseline comparison.

### Bounded debugger localization and pre-fix future sizes

Declared two **diagnostic**, not qualification/retry-to-green, debugger captures of that same isolated database failure. Both use the monitor around `gdb -q --nx --batch`, debuginfod disabled, existing test executable and exact same test filter/`--nocapture`; no source instrumentation, secrets, stack setting or external debugger configuration changed. Auto-load protection stayed enabled; no debugger script was trusted to silence warnings.

- **`storage-stack-gdb1`**,5.082s: stopped on worker **SIGSEGV** before Rust's stack-overflow abort handler. Captured backtrace reaches `btree_automerge::reconstruction::ReconstructedDocument::apply` -> `PendingDocument::new` -> reconstruction stream -> SQL catalog-state/schema reconciliation -> staged SQL synchronization. No repeated recursive frame cycle was captured.
- **`storage-stack-frames1`**,6.340s: captured100 unwind frames with stack pointers and consecutive deltas, avoiding raw arguments/payloads. Major physical SP deltas include **929,616 bytes** between schema reconciliation and its state-transfer caller, **197,424** around SQL sync, **205,392** around Storage exchange, **138,432** around change-subscription polling, **228,768** around Storage run-resource polling, and **139,008** around the spawned job. Inline frames share SPs and are not double-counted. This is cumulative poll-stack pressure, not proof that the top `apply` function alone needs2MiB.

Debugger wrapper exits were0 because batch GDB completed normally after stopping its faulted inferior; they are **not passing test results**. Debugee did not finish the scenario. Read-only DWARF lookup in the same binary measured these pre-fix future layouts, in bytes:

| Future                                             |    Size |
| -------------------------------------------------- | ------: |
| `DatabaseProtocolHandler::exchange`                | 137,312 |
| `DatabaseProtocolHandler::synchronize_peer_active` | 137,696 |
| `DatabaseProtocolHandler::synchronize_peer`        | 138,192 |
| `ReplicationHost::run_resource`                    | 138,496 |

`run_resource` is already boxed at its outer admission boundary, but its nested unboxed SQL-sync await contributes a large state propagated through the intermediate Storage futures and large poll frames. Candidate within owned Storage scope: box that heavyweight SQL synchronization future at `DatabaseProtocolHandler::exchange`'s existing await seam, preserving the awaited operation, authorization/apply hooks and cancellation/drop lifetime. Do not raise worker stacks, detach work or edit sibling SQL implementation. Actual isolated red plus size/frame evidence precedes any such change; its efficacy still requires measured future-size reduction and green. No claim yet that a guessed box fixes the observed failure.

### Follow-up receipts so far

All stages above had no monitor guard stop. Prefixes resolve to `/tmp/setup09-qual-<prefix>{.json,.log,-samples.jsonl}`. Fixture-green compilation sampled minimum available9,764,228KiB, max owned RSS2,436,012KiB, memory PSI avg10 some/full4.94/4.20; both debugger captures stayed above10,085,224KiB available. No unrelated process was killed, caches removed, or sibling files changed.

| Prefix                          | Log SHA-256                                                        |
| ------------------------------- | ------------------------------------------------------------------ |
| `storage-fixture-red-admitted1` | `d1ae620b26baac7fa3b8ff97b0d6f5c7db97ebd13ee3f0486923852c056c3ec6` |
| `storage-fixture-red-queued1`   | `99b5d127467ef147615b8dcf048189a821537665e2bb465cf5a31e20924de89a` |
| `storage-fixture-green1`        | `5b5853969fcfbd111c7aae2679370a6103f4bec4fecf0a232823b51428c36e2e` |
| `storage-stack-database1`       | `c3ef15afd189bcbccb4db6575326483eb17439e2962ad7b0b92ca84fd2fde374` |
| `storage-stack-responder1`      | `d27f0fe8de55cfb2548af553587c76c0a0f596fba950089299c121579aa335f0` |
| `storage-stack-invalidation1`   | `2bcb55313d4bc56844f5af34dfc96f9db47e97334dfdee0756fd3257100421fb` |
| `storage-stack-gdb1`            | `a7947858c842a4366e6e6d10d9461777eefa273eb196c7351abb3044da00fea1` |
| `storage-stack-frames1`         | `d26bd3edaccf8e6c24dc2105fe3b90e26355452d6feac440ab91dba0be451daf` |

### Validated fixture commit; unsuccessful stack candidates removed

Committed the two-line fixture prerequisite repair plus its evidence separately as **`5befc783b4f1f01d96862469f8a9c26bc08d433e`**, hooks disabled. Denial production fix remains independently committed as `366afcd6e`. The following stack experiments happened afterward and **none is retained or committed as production code**:

| Receipt prefix                  | Changed candidate, before test                                                                                   | Actual test result                               | Supervised seconds |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ | -----------------: |
| `storage-stack-green1`          | Box only SQL synchronization await in `DatabaseProtocolHandler::exchange`                                        | **still worker stack overflow/SIGABRT/Cargo101** |            136.045 |
| `storage-stack-green2`          | Box both SQL and KV synchronization awaits in `exchange`                                                         | **still worker stack overflow/SIGABRT/Cargo101** |            126.222 |
| `storage-stack-dispatch-green1` | Restore original `exchange`; box the three mutually exclusive selected-resource await branches in `run_resource` | **still worker stack overflow/SIGABRT/Cargo101** |            134.096 |

Despite the historical `green` prefix names, **none of these is green**. Each invocation followed a different source candidate derived from the preceding frame/layout evidence, not an unchanged retry to obtain a pass. Stopped after these unsuccessful candidates and restored exact committed production source; no guessed stack fix, allocation layer, stack increase, unrelated architecture change or sibling edit remains.

Read-only GDB DWARF layouts refined and limited the initial hypothesis:

- SQL-only boxing left all four measured sizes unchanged. `exchange::Suspend3.__awaitee` was the subsequent **KV** `synchronize_checked` future, **136,672 bytes**, dominating the enclosing future's layout even during SQL polling. This is why SQL-only boxing did not shrink the measured wrapper.
- Boxing SQL and KV reduced `exchange` **137,312 ->1,224**, `synchronize_peer_active` **137,696 ->1,608**, `synchronize_peer` **138,192 ->2,072**, but `run_resource` only **138,496 ->113,984**. Its `Suspend2.__awaitee` held the alternative filesystem `StorageProtocolHandler::synchronize_peer` future, **113,680 bytes**. The test still aborted.
- The dispatch-only candidate restored the original database adapter, reducing `run_resource` to **416 bytes** while the database wrapper sizes returned to their original values. The test still aborted.

These measurements show large inline alternatives and cumulative poll frames, **not a proven sufficient root-cause repair**. Do not equate smaller future storage with a safe poll-stack budget: outer boxing does not by itself remove every callee poll/temporary frame, and sibling SQL/schema reconstruction contributed substantial captured stack. The exact remaining dominant construction/poll boundary needs further isolation before another production change. No combined/all-boundaries candidate or sibling fix was attempted. The explicit2MiB regression remains the correct causal gate; increasing `RUST_MIN_STACK` cannot override that explicit worker stack and is not a fix.

### Final fixture-only source validation and remaining gates

After removing all exploratory production edits, rebuilt the **actual final fixture-only source**:

- **`storage-fixture-final1`**: same common Cargo prefix, `--lib filesystem_transport::execution_authorization_tests:: -- --nocapture`; **2 passed /86 filtered**, exit0,124.704s supervised/4.14s test. This reconfirms the committed prerequisite repair, not a repetition offered as evidence of a stack fix.
- **`storage-denial-final1`**: same common prefix, `--test identity_policy -- --nocapture`; **4 passed**, exit0,4.978s supervised. Current-policy/wrong-owner/missing-bearer/identity-outage/response-recheck controls remain green.
- **`storage-fixture-clippy1`**: common Clippy prefix plus `--lib --tests --no-deps -- -D warnings`; **failed**,15 diagnostics in unchanged code, exit101,7.651s. Did not fix unrelated lint or add suppressions. Exact existing locations/categories: `data_protocol.rs:114` items-after-test-module; `database_execution_tests.rs:729,739` drop-non-drop; `device_client.rs:445` cloned-ref-to-slice-refs; `device_client.rs:729,761,795`, `idp_client.rs:600,631,664`, `router/resources.rs:524` unused-IO-amount; `replication_runtime_tests.rs:619,636,654` needless-range-loop; `router/storage_socket.rs:205` manual-async-fn. None names the modified fixture. Retain this full-test-target Clippy failure independently of scoped success.
- **`storage-fixture-scoped-clippy1`**: common Clippy prefix plus `--lib --test identity_policy --no-deps -- -D warnings`; **passed**, exit0,0.547s. This selects production library/integration target rather than asserting full unit-test Clippy is clean.
- Rustfmt checks passed for `execution_authorization_tests.rs`, both restored candidate modules, `idp_client.rs` and `tests/identity_policy.rs`; `git diff --check` passed. `database_protocol.rs` and `replication_runtime.rs` are byte-identical to the committed integration versions.

Did **not** rerun the broad88-test lib workload after the isolated stack abort remained unresolved. That would repeat a known abort without a new validated fix. Full feature matrix, CRAP, coverage and SDK/HTTPS workloads remain not run in this follow-up; no broader acceptance or historical-timeout attribution is supplied.

Final binary rebuilt from fixture-only source: `target/debug/deps/storage_server-eeafeffb4be7bf1e`, SHA-256 **`dc4be6140f8185e87067609040c8e74bd175a2c60d12b3ab8c5cc6c5e2ca89ee`**. Cargo.lock hash remains unchanged. Intermediate candidate binaries at that same path were overwritten; stage receipts describe their source deltas/outcomes, not retained executable identities.

Additional log hashes (same receipt/log/sample convention):

| Prefix                           | Log SHA-256                                                        |
| -------------------------------- | ------------------------------------------------------------------ |
| `storage-stack-green1`           | `3bb9160cf0350875b8989a14470f1d7a1d893489ba0ca93c538d9a00089054c5` |
| `storage-stack-green2`           | `c43188bc6f527eb6e78ac4a6fc7f78ec60422140405327972851fc08937b093b` |
| `storage-stack-dispatch-green1`  | `b0ad147b844976507dc4e5317f140ee995933987cad987385fbcc6f1bc77bbd0` |
| `storage-fixture-clippy1`        | `b2e743c3ddd36cd3ece47d497df0f1e1e89fc8419d269b591d019c892317c0bc` |
| `storage-fixture-final1`         | `d33ed7ccfa8df26e860a08bad57e30f3f30c32f4524691d60268c6ac24cdf448` |
| `storage-denial-final1`          | `821088d2fc070a9aecaa4b7837618b936e4f66acfa5aa304b18c1536bce96f20` |
| `storage-fixture-scoped-clippy1` | `97c8f8c371a0f488a0f351eed2af1f82b50f31db65964a6c720d45c6db33eaf6` |

All15 follow-up stages completed without a RAM/PSI/stage stop. Minimum available RAM **9,676,760KiB**, peak sampled owned RSS **2,978,844KiB**, largest reported kernel individual-child RSS **3,255,684KiB**; max sampled memory PSI avg10 some/full **4.94/4.20**, below sustained guard thresholds. **Debugger monitoring limit:** GDB placed its inferior in a separate process group, so the saved owned-group RSS samples cover GDB, not combined debugger+inferior RSS; global MemAvailable/PSI guards still measured the host. No debugger resource stop occurred. Explicitly verified both every recorded supervisor-owned PGID and the debugger inferior/thread IDs from both logs absent at final cleanup. No user-tool termination, cache deletion, bound/retry/sleep modification or out-of-scope edit. Release the Rust slot after committing this diagnostic evidence. Keep the worktree; parent must explicitly authorize further stack experiments. Shared ticket09/tracker remains untouched and open.

## Approved engine-local stage fix: current red/green (2026-10-27 UTC receipts)

### Handoff, inputs and temporary-primary safety

Parent explicitly granted the Rust slot and ofdb schema ownership after the point-lookup slice, including temporary application of our exact candidate diff to **tracked-clean primary ofdb**, conditional on reversing only that diff and verifying restoration. Merged ofdb integration into existing `ofdb/.worktrees/setup-engine-cost` / `implement/setup-engine-cost`: fast-forward `0404c945c7c589609b7c37c741d2d8c48d692029` -> **`2b40a4ef62f9012a1e9f9a6494159a3442ce50c5`**. Merged of integration into existing Storage worktree before baseline: **`ca1f3f0755bf3f98b1e770e26d8a2850b175f801`**. No branch was created, primary switched, dependency patched or symlink repointed. Storage resolves its existing `.worktrees/ofdb` symlink to primary `/home/nathan/code/rust/aicacia/ofdb`.

Primary had **no tracked/staged modifications**. Its pre-existing untracked `.worktrees/` directory is worktree infrastructure and was preserved unchanged, not represented as a newly clean/removed user file. Primary schema SHA-256 before every application and after every reversal: **`e2e5cc0f9edabf5e6c0fa364c0e8086ae28ed066a6b02b3b050d2b2d12076c59`**. Each temporary application checked primary HEAD, source hash and tracked/index diffs, ran `git apply --check`, then applied only the saved one-file schema patch. A Python `finally` block checked and reversed only that patch, verified the exact original hash and empty tracked diff. This happened for all three candidates, including both failed ones. Final primary HEAD remains `2b40a4ef62`; no candidate was merged into primary/root.

Candidate source is owned only in `ofdb/.worktrees/setup-engine-cost/crates/sql-engine/src/schema.rs`. Candidate final schema hash **`cada63d1ca590a4747944b0095a161b92555fd129dd775b88df3b3f87d8e9f4a`**. Of lock SHA-256 remains `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`; candidate ofdb lock SHA-256 **`7bb748d9a80168782fd632f30f98edf1c4ee6f82dcd33c032d8b1652eb5db0c5`**. Both unchanged. Loaded owner AGENTS/glossary and TDD/ponytail. Low code complexity, high transaction/validation risk; preserve every rejection and rollback path. No new dependency, helper, task, test-stack/profile/timeout/retry change or repeated outer boxing.

### Actual current red and measured live-frame baseline

All Storage runs used the previously recorded common network-only Cargo prefix and exact unchanged `replication_runtime::tests::database_changes_from_higher_id_and_new_peers_catch_up_then_stay_idle -- --exact --nocapture`, under the same jobs1/root-target/RAM2GiB-floor/PSI supervisor. Original `on_two_mib_workers` still explicitly uses **two workers, 2MiB each**; `RUST_MIN_STACK=67108864` in the supervisor does not override that explicit setting.

**`storage-schema-current-red1`** ran unmodified primary ofdb `2b40a4ef62` (includes `e904a07` snapshot batching plus `0404c94` point reads), rebuilt and actually aborted with worker stack overflow/SIGABRT, Cargo101,293.121s including compilation. Not a memory/stage stop. Baseline executable SHA-256 **`56ffb3245f33fb6c7bd0a462eeb970fc095b9781ed1ad594cbe98064907e6232`**.

Fresh machine code, not old-source inference: native/Automerge `reconcile_schema_names` poll prologue reserved **`0xe2000 + 0xe58 = 929,368 bytes`**, plus saved registers/return. Its stored future was244,800 bytes, namespace-validation child194,944 bytes. Thus batching/point reads had not eliminated the live reconciliation poll-frame problem. The earlier929,560-byte historical frame was not silently reused as the current baseline.

### Three source-distinct engine candidates, not retries to green

| Prefix                            | Candidate                                                                                                                                           | Actual result                                                                  |                                      Supervised seconds |
| --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------: |
| `storage-schema-namespace-green1` | Only `execution_stage(                                                                                                                              |                                                                                | validate_storage_namespace(...))` inside reconciliation | **overflow/SIGABRT/Cargo101**; poll frame unchanged929,368 | 206.901 |
| `storage-schema-phases-green1`    | Extend that existing helper to related heavy reconciliation phases (dependent cleanup, index discovery/validation, active table/schema enumeration) | **overflow/SIGABRT/Cargo101**, but progressed past catalog into data apply     |                                                 260.135 |
| `storage-schema-columns-green1`   | Above plus existing `table_schema_for` -> `columns` await boundary                                                                                  | **1 passed /87 filtered**, exit0, original idle assertion retained; test11.76s |                                                 205.796 |

The first two `green` prefixes denote intended attempts, **not passing results**. Each changed source based on newly measured frames; no unchanged invocation was retried to obtain green. All existing Storage source/tests, including the2MiB regression, remain unchanged in this slice.

Bounded diagnostic debugger captures **`storage-schema-namespace-frame1`** and **`storage-schema-phases-frame1`** ran each corresponding already-failing executable, not new uninstrumented qualification. Both batch GDB wrappers returned0 after stopping a SIGSEGV inferior; neither is a passing test. Namespace-only kept the244,800-byte reconciliation future and929,368-byte physical poll allocation. Related phase boundaries reduced reconciliation to **147,352 bytes of poll-frame locals** (`0x23000 + 0xf98`), **147,408** including saved registers/return, and97,872-byte stored future. The second capture showed a later data-apply fault at `lookup_table_id` under `columns` and `table_schema_for`, with a **293,328-byte unwind delta** at table-schema polling, plus195,600 in its columns child. This concrete new live frame justified the additional one-line columns boundary within the same schema module.

Final machine code and DWARF confirm the reduction, not just successful scheduling:

| Measurement                                       |                                      Current baseline / observed failing phase |                                                                                                        Final |
| ------------------------------------------------- | -----------------------------------------------------------------------------: | -----------------------------------------------------------------------------------------------------------: |
| Native reconciliation poll-frame local allocation |                                                                        929,368 |                                                                                                  **147,352** |
| Reconciliation stored future                      |                                                                        244,800 |                                                                                                   **97,872** |
| Table-schema poll frame                           |                            captured293,328-byte delta including frame overhead | native codegen clones reserve **152 /120 bytes** locally (208 /176 including six saved registers and return) |
| `table_schema_for` stored future                  | not separately recorded before final boundary; large poll delta captured above |                                                                                                **104 bytes** |

The existing `executor::execution_stage` is non-inlined and returns an erased boxed future: it separates construction and nested polling at the **actual schema phases**, unlike the previously unsuccessful generic outer Storage boxes. The final diff is36 lines in one module (23 insertions/13 deletions), with no helper/architecture addition. It preserves operation ordering, same borrowed transaction, validation, errors and commit/rollback ownership. There are bounded per-invocation heap-allocation/dynamic-dispatch costs at these heavy phases; no throughput benchmark claim is made.

### Candidate-worktree semantic controls and strict validation

After actual Storage green, all following commands ran directly in **candidate ofdb worktree**, not restored primary, using `--locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1` and the same monitor. This proves the candidate source independently of temporary-primary application. Parent network interruption did not cause any duplicate red/green/control rerun; saved receipts established completed results before resumption.

- **`schema-stage-catalog-controls1`**, `cargo test ... -p ofdb-sql-engine-automerge --test catalog_snapshot --test index_schema --test scoped_session --test session --test schema_observation`: **36 passed** (8/2/7/12/7 per respective suite),0 failed/ignored/filtered,454.074s including compilation. Covers malformed/tombstoned/current catalog rejection, index identity validation, hostile catalog/history/recovery fail-closed semantics, retained prior rows on rejected import, same-name schema convergence, dependency ordering, deletion, flexible primary keys and existing outgoing snapshot batching.
- **`schema-stage-rollback-controls1`**, `cargo test ... -p ofdb-sql-engine-automerge --test kernel -- --exact failed_table_lifecycle_batch_rolls_back_complete_state rollback_discards_catalog_and_logical_row_changes catalog_batch_validation_fails_without_partial_mutation`: **3 passed /47 filtered**,50.152s including compilation. All three requested exact tests actually ran; no zero-test selection.
- **`schema-stage-clippy1`**, `cargo clippy ... -p ofdb-sql-engine -p ofdb-sql-engine-automerge --lib --no-deps -- -D warnings`: **passed**,20.797s. Scoped engine/codec libraries, not blanket test/workspace Clippy.
- **`schema-stage-nostd-check1`**, `cargo check ... -p ofdb-sql-engine --no-default-features --lib`: **passed**,55.676s; no dependency/features changed to obtain it.
- `rustfmt --edition 2024 --check crates/sql-engine/src/schema.rs` and diff checks: **passed**.

Committed candidate as **`7aeb1148298f28d087501fc044826f32cb12016e`**, `fix(sql-engine): isolate heavy schema reconciliation poll stages`, hooks disabled, on existing `implement/setup-engine-cost`. Merged/confirmed candidate's integration tip `2b40a4ef62` before handoff. Only schema implementation changed in that commit. The original meaningful public2MiB regression is retained in of, not duplicated or weakened to add a same-shape internal test.

No full Storage88-test rerun, full feature-powerset matrix, CRAP, coverage or broader setup/HTTPS workload was run in this boundary slice. Prior unrelated full-test Clippy diagnostics and historical response/readiness attribution remain independent parent gates. Passing this current deterministic regression and the39 owner semantic controls does not diagnose every earlier timeout or qualify the entire setup matrix.

### Durable provenance, resource supervision and disposition

Saved patches `/tmp/setup09-schema-stage-candidate{1,2,3}.patch` SHA-256 respectively **`08670c032e6ff26e3e7283843fd5ae2229e2b5decc09c479b3a4cefe3ffaeacc`**, **`8a5c39986664de22015ea297a78d104f58b50b813290db24964cd41bddf93a53`**, **`f1603cc5126208512e7f793cfe5e4415b593b3823c9545e055c40a2476a0207d`**. Last patch is byte-identical to committed candidate schema diff against `2b40a4ef62`.

Green Storage executable `target/debug/deps/storage_server-eeafeffb4be7bf1e` SHA-256 **`8a2976d0ac918a387e9f1227c21f98c1a118bcc7adbf3142c036eefcfbc2b8e6`**; final disassembly receipt `/tmp/setup09-schema-final-layout.txt` SHA-256 **`adc501fb95cc60503e2bb26ec2a95589019057edb7d7238dd21cc031df2fbec8`**. That executable was built with the temporary identical primary patch; primary source was then restored. Running Cargo against restored primary before parent merges the candidate would build baseline again, not silently use the candidate as current primary source.

All10 monitored stages completed with **no RAM/PSI/stage stop**; min available RAM **8,814,580KiB**, max sampled owned RSS **2,669,980KiB**, max reported kernel individual-child RSS **3,254,416KiB**, peak memory PSI avg10 some/full **3.16/2.61**. Same550s stage ceiling,2GiB RAM floor and sustained PSI guard, full swap not a veto. Debugger own-group RSS excludes its separately grouped inferior, as previously disclosed; host memory/PSI guards remained global. Verified no active owned groups or saved debugger inferiors before final handoff. No user-tool kill, cache deletion, symlink/config/dependency/profile change, primary commit/switch or out-of-scope edit.

Receipts use `/tmp/setup09-qual-<prefix>{.json,.log,-samples.jsonl}`; exact argv/PGID/exit/resources are in JSON. Log SHA-256:

| Prefix                            | SHA-256                                                            |
| --------------------------------- | ------------------------------------------------------------------ |
| `storage-schema-current-red1`     | `3fcfbc97c421b2d758f51b5dc9605ce31a4f400fa11a659465cc238a66f11bdd` |
| `storage-schema-namespace-green1` | `4fa4d86e28c199bbe3e5d9144fc920abbe5b8607d3a981d802a0fab7b0f87a59` |
| `storage-schema-namespace-frame1` | `29e154dfc65de4bfd317b95334d3b081ee1c15a8bac3c4863596fa68ada69a90` |
| `storage-schema-phases-green1`    | `6b8b13b2950fd56fa6465c34b89b2730f94b3d2fa5a968a76d06d1e6b3ead2ad` |
| `storage-schema-phases-frame1`    | `d49a2ed1ac8aa74b9b425f1c74e8602d7772cc43a83f01d47b07772c86b84b33` |
| `storage-schema-columns-green1`   | `0dc29b3e6b5410570d72ca4d6c54964dc6c82c837d0e91977da604a08fe78560` |
| `schema-stage-catalog-controls1`  | `2b5a8bb52c74f776d609520000adab9eeb137f65a92eedfbacbd55f975f7f5d7` |
| `schema-stage-rollback-controls1` | `14fd51a2a87dde86c52da4ce0028fb7b505ef386575144547543a789062a04af` |
| `schema-stage-clippy1`            | `faab407fcb94dcc438bf78ef0dedafa9f668d0344dbc6469940a27ffaf7c3b37` |
| `schema-stage-nostd-check1`       | `c4094f821770fa4233ccc69e1bf20f5b9f0f166b57b6f8cc9499aa5956e4d1d5` |

Disposition: validated owner-local fix ready for parent review. Commit this append-only execution evidence on the Storage branch, merge/confirm of integration tip there, retain both worktrees, and release the Rust slot. **Do not merge candidate into primary ofdb or root of**; parent owns review/integration and any broader qualification. Shared ticket/evidence09, map and spec remain untouched by this slice.
