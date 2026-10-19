# Management authority cutover — integrated Rust qualification

## Scope and acceptance

Main `of` checkout, branch `integration/ui-driven-node-bootstrap`, starting clean at `ded6b1cfbfff627e02d84188d1591f032dc9c9dc`. This is not the earlier bootstrap-runtime worktree. The parent granted this agent the sole Rust build token; commands were sequential, offline/locked, one Cargo job, incremental compilation disabled, and each terminal invocation bounded at 200000ms. No scenario deadline or stack setting was increased. No feature powersets, CRAP, privileged trust installation, real OS credential-provider mutation, docs/TS/generated-client edits, or ticket closure.

**The full spec remains unaccepted.** These results qualify the focused integrated owner/authentication/Create/Join seams, not independently deployed Create/Join workers (absent), WAN behavior, rendered UI, real OS trust/provider installation, or a deployment/platform matrix. Parent-owned docs/generated-client work remains separate. The existing `33-ownership-boundary-clarification.md` is retained; this file does not overwrite it.

Environment observed: Linux `6.8.0-147-generic`, x86_64 GNU/Linux; `rustc 1.99.1 (58cf8f9c3 2026-10-13)`; `cargo 1.99.1 (5f94df478 2026-08-27)`. Cargo describes this inherited per-package configuration as `[optimized + debuginfo]`; only the overrides shown literally below were applied. No performance SLA or release-build performance claim is inferred.

## Changes and retained owner checks

- Map malformed Management Device endpoint IDs to `ManagementError::InvalidInput`, not a database `QueryError` conversion.
- Inject the same already-bound `Arc<HostedControlPlane>` from UnifiedRuntime into NodeJoinSource. Source admission refresh uses that Management owner; no second runtime or cross-owner database open was introduced.
- Preserve ordinary bearer verification and IdP-route audience checks. The existing permission guard now shares an explicit expected-audience implementation; Source Join requires the **Management audience** for both exact `DevicePairingUpdate` and separate `ReplicaSignersEnroll` checks. A valid IdP-audience User token with both grants cannot authorize Source Join. Enrollment/reply rechecks remain in place.
- Move the remaining pre-import Device-binding check out of IdP's `check_node_join_local_binding`. The coordinator calls IdP for retained signer/identity evidence and Management's `DbDeviceRepo::check_pending_approved_binding` for Device evidence. Absence can mean pending import; present contradictions, endpoint aliasing, conflicts and learned revocation deny. Post-import strict trust/readiness checks remain separate.
- Migrate Source Join test paths/audiences to `/management/node-joins`. Add real HTTPS wrong-audience refusal and retired IdP-route absence assertions. Management pairing-generation tests now use Management's actual pairing route, not a compatibility route.
- Update Management Device/selection-policy persistence fixtures to initialize Management schema, and Management service Client fixtures to request `idp.user.lookup` as well as token validation. No production authorization was weakened to accommodate fixtures.
- Fix moved-runtime test handles and explicitly release two shadowed Management test-service handles before stopped reopen. This fixes the fixture's retained database lock, not the production shutdown contract.
- Fix native Rust imports using existing Management exports and the current User-lookup scope. No new dependency was added.
- Retain test-only stage diagnostics identifying public-bootstrap failure before public readiness. No secrets are intentionally emitted.

## Results

All times below are Cargo/test-harness reported; compilation is distinguished from scenario execution. Parent/child counts refer to separate process harnesses, not two independent scenarios. Ignored children were explicitly launched by the passing parent with isolated native CA roots and mock providers.

| Focused seam                                                                                   | Observed final successful result                                                                                                                                              |
| ---------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Unified production/test-target compilation                                                     | Pass, 8.53s final invocation                                                                                                                                                  |
| Native `lidp --tests` compilation                                                              | Pass, 3.80s after two stale import fixes; final changed-source repeat passes in 11.34s                                                                                        |
| Four affected owner/composition library Clippy packages, warnings denied                       | Pass, 9.61s                                                                                                                                                                   |
| Management Device/API authority                                                                | 7 pass, 5.33s tests; 1m16s compilation on final invocation                                                                                                                    |
| Management Device persistence                                                                  | 13 pass, 6.34s; includes pending import, exact owner/name, malformed endpoint, learned revocation, conflict-clear authority, endpoint uniqueness and durable enrollment tests |
| Management selection/transfer policy persistence                                               | 12 pass, 12.40s; includes restrictive conflict resolution, learned-owner cleanup fence and offline reopen                                                                     |
| Source Join exact audience + both permissions + disconnected-reply retry                       | 1 pass, 23.06s; 40.96s compilation on final invocation                                                                                                                        |
| Source Join owner-interleaving/revocation/reply-size rechecks                                  | 3 pass, 33.98s                                                                                                                                                                |
| Production API Create + authentication + stopped/offline reopen                                | Module: 4 pass / 1 deliberately ignored child; child explicitly launched and passed. Parent 14.98s, child 14.34s                                                              |
| Production API Join + authentication + source-offline reopen                                   | Final parent 1 pass, 49.06s; child 1 pass, 48.99s. Includes HTTPS IdP-audience refusal and retired-route absence assertions                                                   |
| Production API Join interrupted retry + rejection                                              | Parent 1 pass, 49.57s; child 1 pass, 49.53s                                                                                                                                   |
| Whole production HTTPS Create/Join + source-offline reopen                                     | Parent 1 pass, 76.63s; child 1 pass, 76.55s                                                                                                                                   |
| One-listener/three-prefix/shared-endpoint composition + learned authorization + stopped reopen | 1 pass, 81.80s after releasing both fixture-held Management services                                                                                                          |
| Real HTTP policy/transfer/copy-retirement chain                                                | 1 pass, 40.12s after fixing the Client fixture's User-lookup scope; 32.48s compilation                                                                                        |
| Existing independent Management permission HTTPS relationship                                  | Parent 1 pass, 23.43s; child 1 pass, 23.39s. This is authorization qualification, **not independent setup-worker qualification**                                              |
| Changed-file rustfmt, `git diff --check`, `just boundary-check`                                | Pass after fixing one native test import-order formatting difference. Boundary output: `No direct server-to-server dependency edges found.`                                   |

### Stage timing observations

Final measured Create: accepted 1ms; endpoint 19ms; IdP 3602ms; Management 3266ms; runtime 149ms; setup worker 7381ms; observed ready 7896ms. Stopped reopen observed ready 1453ms. These are isolated mock-provider/native-root test observations, not real provider/trust installation costs or latency acceptance.

Final API Join: public bootstrap 21518ms (IdP import 9409ms, Management import 7222ms), login Client 1078ms, runtime 454ms, publish 781ms, completion 443ms. Whole HTTPS scenario's public bootstrap was 22561ms, login Client 1146ms, runtime 554ms. Content/catalog checks occur separately from fresh Create and public-state import.

One earlier **successful** API Join run logged repeated transient `endpoint is not allowlisted` refusals before reconnecting and completing public bootstrap in 26965ms, within the unchanged 30s bootstrap retry bound. The final run completed bootstrap in 21518ms without those refusals. The cause of that transport observation was not established; no claim is made that transient admission behavior or latency is fully qualified. No timeout was increased or refusal bypassed.

## Failure and retry record

These are real failures/timeouts, not passes:

1. First unified check after the reported endpoint parse fix and control-plane injection: E0382 for moved `runtime` in two bootstrap test files and moved `restored_idp` in the runtime test. Next check found another branch using the moved runtime. Preserve an Arc where later branches need it, and release the retained control-plane handle at cleanup; subsequent checks pass.
2. First IdP regression command exposed unresolved `secondary_device` / `secondary_endpoint_id` in the integrated live signer test. Replace stale Device-fixture references with independent signer member/endpoint identities. No Device authority was restored to IdP.
3. TDD red: after adding Management-audience coverage, the Source Join test failed at `IdP audience cannot authorize a Management source Join, even with both grants` (10.47s tests after 24.65s compilation). Explicit-audience permission evaluation made it green (22.49s initially; final 23.06s).
4. Initial API Join failed after approval with `host-owned Join stopped unfinished` (18.84s parent / 18.58s child). Test-only stage diagnostics isolated `Invalid query: Table not found` (16.54s / 16.45s): the residual IdP Device query. Split owner checks as above; API and whole Join then pass.
5. The one-listener composition test initially failed stopped Management reopen with `Database already open. Cannot acquire lock.` (74.49s). Releasing only the later service left the earlier shadowed service alive and still failed (77.88s). Releasing both test services made the same unchanged-bound scenario pass (81.80s).
6. Native compilation initially failed on the unavailable production `management_service` import, then on removed `IDP_DEVICE_LOOKUP_SCOPE` in a test. Use the already-linked Management server constant and current `IDP_USER_LOOKUP_SCOPE`; check passes without adding dependencies.
7. Real policy HTTP chain initially failed `IdP rejected the Management service client` (13.03s). Its fixture lacked current `idp.user.lookup`; correcting the exact Client scope made the real chain pass (40.12s).
8. Independent HTTPS permission regression initially failed `AlreadyExists` creating Management's fixture directory (10.81s parent / 10.75s child). Shared seeding now already creates that owner; remove duplicate directory/schema initialization, then pass (23.43s / 23.39s).
9. Initial standalone Management persistence test command selected **zero tests** because default features exclude replica. This is not qualification. The explicit-replica attempt hit the 200s tool bound while rebuilding its narrower dependency graph. Selecting Management together with the existing unified test graph enables its actual replica tests without a feature-powerset sweep; nonzero passing counts are recorded above. Inherited narrower-graph warnings were unused imports in IdP service authorization/scope and dead `ofdb-sql::error::from_engine`; no unrelated cleanup was performed.
10. Tool-bound compilation timeouts: Create once during Management compilation; API Join twice after roughly 3m12s / 3m08s compilation with child just started; standalone explicit-replica once during dependency compilation; combined Management Device tests once compiling the service test; combined selection-policy tests twice compiling the service test. No result was inferred for interrupted scenarios. With no Cargo/rustc/test child left running, commands were retried with exactly the same 200000ms tool limit and unchanged scenario bounds. The initial no-feature zero-test invocation compiled in 3m06s but remains non-qualifying.

The interrupted/rejection scenario intentionally logs an empty-reply JSON parse refusal before exact retry; that expected denial is not repaired or weakened.

## Literal commands

Commands are expanded literally below rather than using shell variables or profile aliases. Repeated invocations used the identical command; their red/green/timeout histories are recorded above. Working directory was the main `of` root. All Cargo invocations used a 200000ms terminal bound.

### Unified test-target check

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo check --offline --locked -p unified-server --tests --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0'
```

### Source Join exact authorization / retry

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-server --lib node_join_requires_both_permissions_and_retries_after_disconnected_reply --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.idp-server.opt-level=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Source Join interleaving and current Device rechecks

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-server --lib node_join_review_tests:: --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.idp-server.opt-level=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Management Device/API authority

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-server --lib device_authority_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Non-qualifying standalone Management zero-test invocation

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-service --lib replica::device_repo::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Timed-out standalone explicit-replica invocation

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-service --features replica --lib replica::device_repo::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Management Device persistence

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-service -p unified-server --lib replica::device_repo::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Management selection/transfer policy persistence

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-service -p unified-server --lib replica::selection_policy::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Production API Create and recovery

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib api_create_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Production API Join / source-offline reopen

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_api_join_completion_and_offline_reopen --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Whole production HTTPS Create/Join / source-offline reopen

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_https_create_join_and_offline_reopen_with_isolated_native_roots --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Production API Join interrupted retry / rejection

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_api_join_interrupted_retry_and_rejection --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### One-listener runtime composition / stopped reopen

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib builds_three_prefixed_services_with_one_endpoint --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Real policy HTTP transfer / copy cleanup

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib real_policy_http_chain_retires_copies_reselects_generation_and_gates_transfer --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Independent authorization relationship, not independent setup

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib independent_permission_production_management_https --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Native production/test-target compilation

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo check --offline --locked -p lidp --tests --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0'
```

### Initial unified-library Clippy

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo clippy --offline --locked -p unified-server --lib --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- -D warnings
```

### Affected owner/composition library Clippy

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo clippy --offline --locked -p management-service -p idp-server -p management-server -p unified-server --lib --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- -D warnings
```

### Changed-file formatting

```sh
rustfmt --check --edition 2024 --config skip_children=true apps/app/src-tauri/src/service_relationship_tests.rs apps/app/src-tauri/src/setup_peer_revocation.rs crates/idp-server/src/cli/provision_service_client.rs crates/idp-server/src/node_join_bootstrap.rs crates/idp-server/src/node_join_review_tests.rs crates/idp-server/src/node_join_tests.rs crates/idp-server/src/router/middleware/identity_administration.rs crates/idp-server/src/router/middleware/mod.rs crates/idp-server/src/router/node_join_approval.rs crates/management-service/src/replica/device_repo.rs crates/management-service/src/replica/selection_policy.rs crates/unified-server/src/api_join_tests.rs crates/unified-server/src/https_bootstrap_tests.rs crates/unified-server/src/independent_permission_http_tests.rs crates/unified-server/src/local_join.rs crates/unified-server/src/node_bootstrap_interruption_tests.rs crates/unified-server/src/node_join_bootstrap_tests.rs crates/unified-server/src/node_join_source.rs crates/unified-server/src/permission_http_tests.rs crates/unified-server/src/policy_cleanup_http_tests.rs crates/unified-server/src/runtime.rs crates/unified-server/src/security_bootstrap.rs
```

### Whitespace validation

```sh
git --no-pager diff --check
```

### Server dependency boundaries

```sh
just boundary-check
```

## Source provenance

Starting tracked main checkout was clean. The following SHA-256 values identify the final changed Rust sources and unchanged lockfile used for this receipt. Evidence is generated after testing, so its own hash is intentionally excluded. Final production Create/Join qualification follows the owner/authentication fixes; later fixture-only fixes are explicitly qualified by the corresponding tests above. No tickets or generated clients are included in this patch.

```text
89b1b74185e581b8bf347bab82767a424ef50bc2812c6352ecf3f41aa6bda63d  Cargo.lock
b4001da6dc9e6838956d3ce76dd82fc49eaa1057bc527aaab03a084672795c36  apps/app/src-tauri/src/service_relationship_tests.rs
b8ee307934fab0c62e7732f3f16fb3ae7860370e9de9435e875a18f1b0dbe22a  apps/app/src-tauri/src/setup_peer_revocation.rs
ea129a331cff275842abde29c24bd620c7bc88d781e268378bf244463d557f83  crates/idp-server/src/cli/provision_service_client.rs
1da1ddcbb10234aff4e32b9c52a5133e81973f2a1a68fc486a031bc9820330ed  crates/idp-server/src/node_join_bootstrap.rs
bb233eab861bc9d2b801f0e30fcbb1b5800cc421eec707f752cc5a0dfae69346  crates/idp-server/src/node_join_review_tests.rs
e795c56825ec57c8b3007070de7779c496e4ce4b7efe894b32022a3d35f9ac19  crates/idp-server/src/node_join_tests.rs
4cf6cfc1960044069b7cb79662c8dbd0fb67ecaff9013f6d151a52920c2ae96d  crates/idp-server/src/router/middleware/identity_administration.rs
7acd387c5ce56f5d8c0777e53e9b6dd422ea0f195ce32faeb5570b20555d0c25  crates/idp-server/src/router/middleware/mod.rs
56839ca9eca78f83990aadfdb6afee020df5b73bd25a803048ca05d60bbd31ed  crates/idp-server/src/router/node_join_approval.rs
2555082e70803f2cc04bec2de58cf94c167abbfbfb310c1c24b8b155a72a8045  crates/management-service/src/replica/device_repo.rs
3d61045e6cdbbb6de6af5d0c5467930ef6be8e6d43996739a4bd55fb0e5397c8  crates/management-service/src/replica/selection_policy.rs
0fc7e4e61e2b16ca7c4ed8ad9a3a7559b43e1f39fd8b560587451eeb10b42557  crates/unified-server/src/api_join_tests.rs
cfc8c268274dd1c530d39671a337ec7a4177223bb19eb2dd00948840778b7a82  crates/unified-server/src/https_bootstrap_tests.rs
6858622407a42d1c042bb5d26505555451d77595482b01c18e935b0fd84af421  crates/unified-server/src/independent_permission_http_tests.rs
cecb3aea51d0a6b0586d86bfa6964159f2733dc3c98184b0b1b4703aaeac7095  crates/unified-server/src/local_join.rs
3d950caa4e3845bcb859f35cdf6a14751bd5b1ede2b83e7598a18db9497249e1  crates/unified-server/src/node_bootstrap_interruption_tests.rs
5007a8aa61ae1c229aa5c1fdf5facdc9f64bfb1f19a7a9e05f1881f8fda70844  crates/unified-server/src/node_join_bootstrap_tests.rs
ffb13418724032997e00c1c4838b3ddf260932f43e3c01ed09f2cf56c8d4715b  crates/unified-server/src/node_join_source.rs
844ec2d60ba56a2c08f7ecf84380fb2cdf8d6d64d6d27719f65eb756cfd619f6  crates/unified-server/src/permission_http_tests.rs
561e8f960a402562a732c496962451bf900c89d052f2108381eee0e4b1a3003e  crates/unified-server/src/policy_cleanup_http_tests.rs
ce2a602c82335cb2f386d11ba94916b45539a6ed423aee3eac8048db52b796fd  crates/unified-server/src/runtime.rs
50d469ec0642249789daa602800e667a3614b427864dc11dc2ef59d487c6b538  crates/unified-server/src/security_bootstrap.rs
```

Path dependency checkout provenance (commit and tracked/untracked status; read only). The dirty `ofdb` working tree was not modified or committed by this agent. Qualification is for the hashed working tree, not a clean multi-repository release; in particular its untracked sharded-BTree implementation is part of the recorded source inputs:

```text
../ofdb: 238210c5b822cd8b5f14274ea63d941ef884f6a0
M Cargo.lock
 M crates/btree-automerge/src/automerge_tree.rs
 M crates/btree-automerge/src/document_change_key.rs
 M crates/btree/Cargo.toml
 M crates/btree/src/btree.rs
 M crates/btree/src/in_memory_btree.rs
 M crates/btree/src/lib.rs
?? crates/btree/src/sharded_btree.rs
8947ae402cbd5e62e98d6fff91e1d47eae7e1e89a9e82bb84ad1f9102090a5d5  ../ofdb/Cargo.lock
43c09063ebd3692f26e2510a1ce81e4d0b30df5bd13e242c10c2c03db8a00207  ../ofdb/crates/btree-automerge/src/automerge_tree.rs
b8a3dd3a03ac7f12926c15e0b344c4224178d27e0538b9146c20cb2c793fe962  ../ofdb/crates/btree-automerge/src/document_change_key.rs
e1d3782dec0662d629d0adfa46cf71e91dd32cdaaf5624ab7ebc82fb31ea754f  ../ofdb/crates/btree/Cargo.toml
21878f7287f9e38996ffacb53b21c1a7e2617d214d6022a267f93fadb7ffad16  ../ofdb/crates/btree/src/btree.rs
ed8028b1129bfb31e1f5b9cc73ee4b98db009ad5938bd9e2516bd65bab4dc78a  ../ofdb/crates/btree/src/in_memory_btree.rs
aa765b29cc88de62c6907a0e3f6a04f35be9353fb5609c93be728b3bb3a54a4f  ../ofdb/crates/btree/src/lib.rs
8bed1c2d8a52ca9c17eec00d1ac406e337f9b98147499fe8d8a1fea6405cc36d  ../ofdb/crates/btree/src/sharded_btree.rs
../offs: 614f2e5f80d4184a67de6fc43a7d26b6e1000714
(clean)
../ofnet: a001f45836393d61a98652294b6d9ea9a73dfcaa
(clean)
```
