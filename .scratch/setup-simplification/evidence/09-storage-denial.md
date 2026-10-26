# Ticket09 Storage denial slice

**Current status:** implemented with actual public-API red/green and focused controls. The earlier source proposal, RAM-stop and WIP sections below are historical receipts, superseded only where the [authorized resumption](#authorized-resumption-and-implementation-2026-10-26) supplies actual results. The broader Storage library run failed/aborted; this is not full-suite qualification or ticket09 closure.

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
