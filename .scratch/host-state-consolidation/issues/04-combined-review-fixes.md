# 04: Close verified combined review findings

Status: resolved
Type: task
Blocked by: 01, 02, 03

Fix all five combined review findings against integration `69ddbdd57`, using the spec-approved Unified lifecycle, injected provider and essential SQL fault seams. High priority, medium complexity, high recovery/security risk: do not orphan provider secrets, replace identities, lose canonical choices or downgrade SQL retirement. Reuse Create's authoritative-read pattern; no framework or compatibility state.

## Scope

1. Refuse Create reset after loss of provisioning/endpoint/domain directories while authoritative SQL Create evidence survives; retain exact SQL/provider references.
2. Require Join's endpoint reference field to be present, including explicit null before attempts.
3. Restore and corrupt the actual host SQL in the stopped same-Node recovery test; compare complete logical SQL and full filesystem bytes without binary dumps.
4. Remove the cached Join journal and stale-completion exception; read authoritative state before decisions and mutations.
5. Derive Join peer/Node choices from the canonical operation, including standalone Join; preserve hosted nonce and issuer/config validation.

## Validation plan

Focused red/green regressions, affected Unified lifecycle/provider tests, hosted/API Join wrappers, SQL/config/host integration targets, changed-file rustfmt, affected-crate no-dependency clippy and diff checks. No full feature powerset, CRAP, live providers, real OS trust or push/main changes.

## Answer

All five findings were verified and fixed on `implement/host-review-fixes`, based on integration `69ddbdd57`, by one implementation agent.

- **Reset loss:** `BootstrapConfig::local_reset_references` now consults retained SQL Create evidence even when provisioning, endpoint and domain directories are absent. It refuses rather than returning `keys: null`. This deliberately conservative option requires exact directory recovery before reset; it does not attempt to validate missing inventories or reconstruct references. The injected-provider regression creates real endpoint material, removes both directories, and verifies refusal retains the SQL references and exact provider secret.
- **Required Join reference:** Join uses Create's `deserialize_with = "Deserialize::deserialize"` on `endpoint_secret`. Explicit null remains valid before an attempt; an omitted field refuses before endpoint/provider work. The regression compares the entire logical SQL snapshot and verifies zero endpoint-provider calls.
- **Real stopped recovery:** the snapshot/displacement/restore set now includes `endpoint.host`, asserts it was actually displaced, compares all host records logically and proves the original canonical operation/allocations survive restore. Host redb files are compared as complete logical records, all other files retain full byte/mode/tree checks with bounded mismatch diagnostics. The corrupt-journal scenario now corrupts the SQL Create record and verifies intact refusal, not an obsolete `intent.json` file.
- **Authoritative Join:** the cached journal and stale-completion exception are deleted. Decisions and updates read validated SQL, including after runtime preparation and before completion. Repeated `complete_hosted` cannot downgrade retired completion. The hosted lifecycle regression holds the same guard through retirement, repeats completion without changing SQL, stops the approving peer, and prepares that same guard offline without restarting owner work.
- **One choice owner:** peer and Node name are removed from allocation metadata. Standalone creation atomically binds a canonical Join and allocations; hosted creation preserves its already-admitted canonical nonce and validates peer/name/issuer against configuration. Retry and protocol requests derive choices from canonical SQL. The standalone/hosted regression verifies no duplicate fields, exact retry, retained hosted nonce and intact issuer-conflict denial. `LocalJoin::trusted_peer` and `login_client` now return fallible authoritative reads; all call sites were updated. No framework, compatibility state, dependencies or provider fallback was introduced.

### Focused validation

Commands run from the requested `host-review-fixes` worktree, with isolated mock/injected providers and native-root HTTPS child fixtures. All final commands below passed.

| Command | Result |
| --- | --- |
| `cargo test -p unified-server --no-default-features --lib local_provisioning_tests:: -- --test-threads=1 --skip https_bootstrap_tests` | 36 passed; final repeat 188.84 seconds. Includes reset/provider/omitted-field, stopped SQL recovery, canonical Join, Create and owner-readiness regressions. |
| `cargo test -p unified-server --no-default-features --test bootstrap_config --test setup_host --test host_sql --test host_sql_interruption --test host_sql_transactions -- --test-threads=1` | 21 passed (10 + 6 + 1 + 1 + 3). |
| `cargo test -p unified-server --no-default-features --lib hosted_join_cancellation_exact_retry_and_offline_reopen -- --test-threads=1` | Wrapper and isolated child passed; final repeat 106.51 seconds. Covers same-guard retirement/repeated completion and offline preparation. |
| `cargo test -p unified-server --no-default-features --lib production_api_join_completion_and_offline_reopen -- --test-threads=1` | Wrapper and isolated child passed on rerun. |
| `cargo test -p unified-server --no-default-features --lib production_api_join_interruption_preserves_private_host_intent -- --test-threads=1` | Wrapper and isolated child passed. |
| `cargo test -p unified-server --no-default-features --lib production_api_join_atomic_retirement_leftover_code_offline_reopen -- --test-threads=1` | Wrapper and isolated child passed. |
| `cargo test -p unified-server --no-default-features --lib endpoint_provider_tests -- --test-threads=1` | Final focused repeat: 11 passed. |
| `cargo test -p unified-server --no-default-features --lib join_choices_have_one_canonical -- --test-threads=1` | Final focused repeat: 1 passed. |
| `cargo check -p unified-server --test full_node --no-default-features` | Passed; compile-only, not full-node execution. |
| `cargo clippy -p unified-server --no-default-features --lib --no-deps -- -D warnings` | Passed, including final repeat. |
| `rustfmt --check --edition 2024 --config skip_children=true crates/unified-server/src/bootstrap_config.rs crates/unified-server/src/endpoint_provider_tests.rs crates/unified-server/src/https_bootstrap_tests.rs crates/unified-server/src/local_join.rs crates/unified-server/src/local_provisioning_tests.rs crates/unified-server/src/setup_runtime.rs` | Passed. |
| `git --no-pager diff --check` | Passed. |

### Red/green and limits

Each behavioral slice was observed red before green with the focused named filter: `reset_refuses_lost_create_directories`, `join_without_endpoint_reference_field`, `hosted_join_cancellation_exact_retry_and_offline_reopen`, `join_choices_have_one_canonical`, and `local_provisioning_restores_stopped_same_node_filesystem_with_retained_provider`. They respectively exposed empty-reset acceptance, missing-field acceptance, completion downgrade, missing standalone canonical binding, and the original SQL directory left live outside the recovery fixture. All are green in the final checks above.

- The first reset test command reached its 240-second cold-compilation bound; rerunning completed compilation and demonstrated the intended assertion failure before the fix.
- The first API completion/offline wrapper failed its existing 15-second pending-Iroh-offer wait while other integration tests were also running. Its unchanged rerun passed; no timing budget, warming, or production behavior was altered to hide the failure. The cause of that initial timing failure is not established.
- Three existing shared-fixture dead-code warnings remain in library test builds.
- No full feature-powerset/hack, CRAP, whole-workspace or whole HTTPS suite, full-node execution, desktop/GUI run, live provider persistence/reboot, or real OS trust installation was performed or certified. No independent-owner module changed. No push or main-branch operation was performed.
