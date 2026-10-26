# Ticket09 Storage denial slice

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

This source mechanism and proposed regression do not establish the cause of every historical Storage timeout. Archived failing requests show overlapping dependent token work, but do not identify the exact completion time of identity rejection versus policy completion. Authenticated Storage timeouts, remote readiness, permission requests and Join history/readiness failures remain separate questions. No full-suite reliability, historical common cause or ticket09 closure is claimed.
