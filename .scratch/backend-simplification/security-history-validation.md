# Independent build and validated-history repair

Branch: `backend-simplification-buildfix`.
Merged integration `8596dfa08` by fast-forward before implementation.
This record supersedes the independent-build and fail-all security-history limits in `buildfix-validation.md`.

## Implementation

- Removed the added `independent` feature gate. IdP and Device independent lifecycle modules compile without CLI/default features. Their normal independent CLI binaries also compile.
- Enabled the actual independent join-history module. Migrated it to the checked native history API. WebSocket client/server identity now determines Initiator/Responder roles; IdP/Device context no longer incorrectly determines the transport role. Reverse-flow checks, exact signed approval checks, cancellation, and single-use admission remain.
- Replaced unchecked security-database push exchange and blanket ChangeEvent/Snapshot rejection with real scoped, staged native-history exchange. Every historical value, fixed schema, merged candidate, and relevant peer/owner binding is checked. Authorization is checked at transport operations and before native apply. Invalid staged data is rolled back rather than applied.
- Migrated node bootstrap to the same checked history API with explicit receiver/source roles and exact join-candidate validation.
- Removed the invented candidate row/table models. Validators now consume actual engine candidate observations, including tombstones, conflicts, decoder column order, and incoming/outgoing direction.
- Updated the User security schema contract for existing MFA/blocked-state columns without changing migrations. MFA columns permit only null historical values. An actual transport test verifies that overwritten MFA secrets prevent disclosure before any payload is sent.
- Re-enabled the existing security history, candidate, outbound/revocation, and valid-exchange tests. No security tests or independent features were disabled to obtain a successful build.
- Reused the actual history byte-transport trait in `db`; removed the duplicate local trait and obsolete stream-conversion method.
- Updated model and Device test fixtures to use atomic native-history imports, not removed transport APIs. Applying catalog rows one at a time to an empty database failed with `Incomplete catalog`; atomic batch import fixes the cause.
- Moved the shared owner HTTPS fixture from deleted-api paths/the setup crate to `server-common`, behind `test-support`. All three owner test callers now use the shared fixture. Setup behavior code is unchanged; only fixture/test-module wiring changed. Removed duplicate root lifecycle-test declarations and placed teardown tests beside their private implementation.
- Fixed stale test references to removed `up_policy` and accidentally renamed `keyring_core::server_common` (the actual external module is `keyring_core::api`).

No local seed, migration, performance, frontend, README, or existing dependency-path edits were made in this repair.

## Required dependency and integration handoff

**ofdb commit `3bda01f58563a0c53b7ce9202f280089c3294eda`** is required.

- Branch/worktree: `backend-simplification-buildfix`, `ofdb/.scratch/worktrees/buildfix`.
- Parent: snapshot-serde fix `503dc08`.
- No ofdb main checkout was modified or merged.
- Adds `ofdb-sql-history`, reusing the bounded staged protocol implementation from the last validated engine implementation (`cf50c07^:crates/sql-sync`) against the current engine's existing `RowReconciler`, complete-history inspection, and atomic validated mutation APIs. This is real native history exchange, not a compatibility facade over unchecked push sync. It preserves the checked history protocol and its recovery/validation behavior.
- `ofdb-sql/validated-history` exposes `ofdb_sql::history` plus native `Database::exchange_history`, `history_manifest`, `export_history`, `apply_history`, and atomic `import_history`. Trusted restore operations are documented separately from checked network exchange.
- `native-stage` now enables this real implementation instead of being an empty feature. `db/network` and model test dependencies explicitly require `validated-history`.
- Existing `ofdb-buildfix`/`offs-buildfix` dependency paths were not rewritten. The dependency-path/sibling-integration agent must integrate this dependency commit, not merely the older `503dc08`.
- Commit hooks were bypassed; the explicit validation below is the qualification record.

## Passed validation

| Boundary / command | Result |
| --- | --- |
| `cargo check -p device-server -p idp-server -p storage-server -p unified-server --bins` | Passed. Independent and unified binaries compile. |
| `cargo check -p idp-server -p device-server --no-default-features --lib` | Passed. Independent paths are not hidden behind CLI/default features. |
| `cargo test -p idp-server -p device-server --all-targets --no-run` | Passed. |
| Device unit tests, across focused runs | **85 distinct tests passed**: 60 other/contract/policy tests, 17 history/candidate/MFA tests, five outbound/revocation tests, one full native exchange test, and two transport panic/denial fixture tests. |
| Device history/candidate tests | First bounded run completed ten passing cases before the 900-second command limit. The remaining six original cases passed in a separate run (619.77 seconds). The additional overwritten-MFA test passed separately. No history case was ignored or removed. |
| Full real scoped exchange test | Passed in 299.25 seconds. Covers identities, replay, secret exclusion, Management namespace, concurrent restrictions, restrictive resolution, tombstones, and reopen. |
| Device outbound tests | **5 passed**, including both roles/contexts, learned grant/User/Device denial, cancellation, and an already-admitted write followed by denial of the next write. |
| Device security contract tests | **3 passed** (included in the 85 unit-test count). |
| `cargo test -p device-service --features replica --test device_contract` | **5 passed**. |
| `cargo test -p device-server --test hosted_node_join` | **2 passed**. |
| IdP `node_join_tests` | **6 passed**. |
| `cargo test -p of-model --features replica` | **24 unit + 6 integration tests passed**. |
| `cargo test -p setup -- --test-threads=1` | **46 passed, 2 existing ignored child/subprocess entrypoints**; compile-fail doctest passed. |
| IdP and Device `production_owner_https_admission_uses_isolated_native_roots` | Both parent tests and their actual isolated HTTPS child cases passed. Test-only trust-command stubs prevent real OS trust changes. |
| `cargo test -p unified-server --test composition fresh_h -- --test-threads=1 --nocapture` | Both actual HTTP and HTTPS four-owner health/sign-in/PKCE/restart/TLS-reuse tests passed. The separate performance test was not selected. |
| `cargo clippy -p device-service --features replica --lib` | Passed with warnings. |
| ofdb `cargo hack test -p ofdb-sql-history --feature-powerset --all-targets` | Passed. All 16 history tests pass with and without native staging. |
| ofdb Clippy for `ofdb-sql-history` and `ofdb-sql` with redb/validated-history, all targets | Passed with warnings. |
| `rustfmt --check --config skip_children=true` on changed Rust files; `git diff --check` | Passed. ofdb targeted package formatting also passed. |

Latest real unified timings: HTTP fresh 22.834 s/restart 8.018 s; HTTPS fresh 22.171 s/restart 7.889 s. Performance remains outside this repair's ownership.

## Exact remaining blockers and limits

1. **Storage all-target test compilation remains broken**, although the Storage CLI binary compiles. `crates/storage-server/src/database_protocol_tests.rs:10` still imports removed `ofdb_sql::SyncTransport`. `database_execution_tests.rs` still expects the removed root SQL `SyncTransport`, `SyncMessage`, `SyncError`, old checked-session APIs, `KvAuthorizedTransport`, old SQL/KV error helpers, and an owning/defaulted `AuthorizedTransport` rather than the current borrowed generic type. Its KV transport implementations also use `Vec<u8>` instead of native typed messages. These native Storage mesh test drivers were not ported in this Device security/history repair. The Storage HTTPS fixture path is fixed, but its unit-test binary cannot be qualified while these mesh tests fail compilation.
2. **Device all-target Clippy** fails on two existing `clippy::unused_io_amount` errors in unchanged `crates/device-service/src/hosted_control_plane.rs:897` and `:906`. The test code ignores the lengths returned by `Read::read`. Device library Clippy passes. No unrelated test behavior was changed to silence these errors.
3. **of workspace feature-powerset tests** timed out after 200 seconds during compilation. `just crap` also timed out after 200 seconds during coverage compilation. No complete workspace/CRAP result is claimed.
4. **ofdb workspace feature-powerset** fails in unchanged `crates/btree/src/sharded_btree.rs` for `ofdb-btree --no-default-features --features sharded`: missing `std`, unavailable `InMemoryBTreeShardFactory`, and missing `ToString` scope. The new history crate's own complete feature powerset passes. ofdb `just crap` timed out after 200 seconds.
5. Package-wide root formatting checks report existing formatting differences in untouched Device/setup modules. Changed-file formatting passes. Missing `api` HTTPS fixture references are no longer the cause.
6. Populated local MFA data cannot be exported as opaque User-row CRDT history. Non-null current, overwritten, or deleted MFA secrets correctly reject scoped exchange. Supporting public User replication from such mixed rows needs an explicit local-secret/schema separation or projection design from the migration/domain owner; no secret-bearing history was allowed through to make a test pass.
7. Full installation-level independent IdP/Device/Storage joining and native Storage mesh synchronization were not qualified end to end. Real scoped security transport, current-policy denial, owner HTTPS setup, signed join behavior, and unified functional smoke were tested as listed above. This is not a production-deployment qualification.

Nothing was merged into integration or main. The final response records the of branch commit.
