# Storage protocol repair validation

Branch: `backend-simplification-buildfix`, based on `200c29056`.
Integration tip `8596dfa08` is already an ancestor. Nothing was merged back into integration or main.

## Changes

- Native push sessions have explicit initiator/responder roles. Finite sessions wait for outbound acknowledgements before closing.
- Storage opens separate concurrent SQL and KV streams. Session arbitration includes the phase, so stalled SQL does not hold the KV permit.
- SQL uses bounded staged resource-history exchange. KV uses validated native snapshots and history, not an unconditional rejection or a compatibility shim.
- Both exchanges check current authorization and cancellation immediately before apply. KV checks inside the transaction before commit; denial rolls back the batch.
- Resource workers use coalesced committed-change notifications after each bounded pass. They do not poll while idle or treat a finite event inventory as a live subscription.
- Mesh tests use current history APIs. HTTP test fixtures consume complete bounded headers and bodies instead of ignoring short reads.
- Canonical OpenAPI exports retain independent setup, join, and history operations. No generated documents or clients were overwritten.

Complexity/risk/priority: high security and replication risk; P1. Separate streams reuse existing framing, policy checks, and collision arbitration. Revocation, rollback, retry, and idle tests cover the critical behavior.

## Required dependency

**ofdb commit `544b6270116ef73e7cab53b2a191bcad608146ba`**, branch `backend-simplification-buildfix`, worktree `ofdb/.scratch/worktrees/buildfix`.

Its parent is performance commit `b8d5088`; validated scoped SQL history is inherited from `3bda01f`. The dependency commit includes native SQL resource-history exchange, checked KV history/import, and role-aware native/Wasm push handshakes. The performance agent's executor/cache/codec files were not edited by this repair. Cargo paths were not changed.

## Passed

- `cargo test -p storage-server --all-targets -- --test-threads=1`: 95 unit tests, 4 identity-policy tests, and 1 local runtime test passed. One HTTPS child entrypoint remains directly ignored; its parent invokes isolated HTTPS child cases.
- The production two-peer test now admits a SQL stream and leaves it stalled, then transfers and verifies KV on a separate stream before closing SQL. It also tests denial and retry on the existing connection.
- Mesh tests cover real SQL/KV exchange, queued sender revocation, SQL blocked pre-apply denial, KV first-batch retention and second-batch rollback, both roles/devices, retry, reconnect, resource retirement, filesystem transfer, and idle behavior.
- `cargo clippy -p storage-server --all-targets`: passed with warnings.
- `cargo clippy -p device-service --features replica --all-targets`: passed with warnings.
- `cargo test -p unified-server --all-targets -- --test-threads=1`: passed. Three real-process composition tests cover fresh HTTP/HTTPS directories, four-owner health, sign-in, client authentication, restart, retained TLS, and the serial three-second readiness case. The canonical OpenAPI export regression test also passed.
- `cargo test -p unified-server --example export-openapi`: passed separately.
- ofdb: `cargo test -p ofdb-sync --features std,iroh`: 7 passed, including a real paired-channel bidirectional exchange.
- ofdb: `cargo test -p ofdb-kv --features redb,in-memory,sync --all-targets`: 12 unit, 1 integration, 3 embedded redb, and 2 local KV tests passed; two existing diagnostic tests ignored.
- ofdb: `cargo test -p ofdb-kv-store`, SQL native-stage history tests, and KV/sync all-target Clippy passed in this repair session.
- Explicit owned-file rustfmt and `git diff --check`: passed.

## Remaining limits and failures

- Parallel `cargo test -p unified-server --all-targets` passed HTTP/HTTPS authentication/restart but failed `fresh_server_is_ready_within_three_seconds`. The serial run passed all three cases. Parallel startup timing remains unqualified; this repair does not change seed/migration/performance code.
- `cargo check -p storage-server --all-features --all-targets` fails in the separate optional WebSocket implementation, `crates/storage-server/src/sync.rs`: missing `tracing`, `rmp_serde`, and `Id`; removed `ofdb_sql::SyncTransport`; and `WsNamespace::application_id` returning `&str` instead of the current UUID type (10 compiler errors). The file also still calls obsolete `get_database` and `synchronize` APIs. It was not disabled or claimed as qualified. Native Storage mesh is green; optional WebSocket sync is not.
- of: `cargo hack test --feature-powerset --workspace --all-targets` reached the 200-second limit during compilation of the first package/dependency graph. No workspace powerset result.
- of: `just crap` reached the 200-second limit while compiling coverage dependencies, including desktop crates; the log ends with `Killed`. No CRAP score.
- ofdb: `cargo hack test --feature-powerset --all-targets` fails in existing `ofdb-btree` sharded-only tests: in-memory shard factory is feature-gated out and no-std tests lack allocation imports (`String`/`ToString`). These are outside the sync/history repair.
- ofdb: `just crap` reached the 200-second limit during coverage compilation. No CRAP score.
- The mirrored Wasm push handshake was not target-compiled. Native tests do not qualify Wasm/browser transports.

No desktop/frontend, README, local seed/migration, or dependency-path changes were made. Validation logs are untracked and are not part of the implementation commit.
