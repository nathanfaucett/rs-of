# 05 — Independent review fixes

Status: resolved
Type: task
Base: `6f9a4cf2a`
Branch: `implement/shared-control-plane-review`

## Scope and seams

Read the original untracked `review.md`, spec and ticket Answers. Original review/notes were not copied or edited. Called `tdd` and `ponytail`; the user approved regression seams for all four findings: authenticated native security synchronization/bootstrap, independent local HTTPS Create/Join/reopen/administration, and remote independent consumer reset with another consumer operating. No new dependencies, compatibility wrappers, sibling-repository edits or original-checkout merge.

Complexity/risk: explicit policy projection/session restoration is moderate security risk; reset deletion removal is small but high-impact; default local identity connection is moderate lifecycle risk. Priority is P1 for all four. Keep database ownership, protected signers, source approval checks and fail-closed authorization intact.

## Answer

### Fixes

1. Identity policy is explicitly allowlisted in `idp-service::security_sync::PUBLIC_IDENTITY_POLICY_TABLES`: roles, permissions, role-permission links and User-role assignments. Public projections validate reviewed columns, historical/candidate value types and UUIDs, including the established nil Installation namespace. Unified sync/bootstrap transfers policy through the identity owner in its separate policy session, not an intersection with Device tables. Independent IdP Join transfers policy with identity history. Device history remains Device-only. No empty policy SQL validation batch remains.
2. `OwnerProvisioner::delete_local_secrets` no longer deletes public SQL key metadata for any caller. It still verifies retained references, detects unexpected private material and removes only exact protected/local inventory. Local owner reset subsequently removes its own database as before; remote consumers never retire shared rows. Public key retirement remains an authorized domain operation, not a reset side effect.
3. Default same-machine Unix independent setup now serves the sole IdP-owned identity database through ofdb-sql's existing Unix-socket adapter. IdP's private setup-host directory contains the socket; exact-operation HTTPS discovery publishes its path. Device uses that query client for administrator provisioning and runtime policy, never opens IdP's live redb file, and rediscovers the socket on reopen. Connect/schema validation is bounded at 60 seconds, with 10-second query deadlines. Clean shutdown removes the socket; retained nonsocket evidence is refused. Explicit remote identity/Device SQL and Unified direct injection remain separate startup paths. Cross-machine/non-Unix deployments require explicit remote SQL.
4. Independent IdP always injects `LocalPermissions` over its identity client, local or remote. Removed Device-as-Management evaluator configuration and its URI fallback. Device runtime accepts an identity client explicitly and reads serving URLs from its existing configuration, rather than independently reopening identity or retaining duplicate URI arguments.

Local Join qualification also found and fixed the leftover `JoinHistoryContext::Device` recipient `management` instead of `device`. Updated affected setup/native history/bootstrap fixtures to seed/query policy in identity. Expanded default local acceptance corrected two stale fixture errors: removed `device.replication.*` scope names in favor of the existing `management.replication.*` authorization identifiers, and stopped shadowing a bearer token with a Device ID. Whole lifecycle child bounds are now 180 seconds, readiness 60 seconds and restricted-restart readiness 75 seconds; no latency SLA is claimed. IdP policy migration EOF whitespace is fixed.

### Red → green evidence

- **Policy:** new `identity_policy_history_grant_and_revocation_reach_an_existing_peer` initially failed with history outside the fixed table scope. Restoring policy scope/validators exposed the legitimate nil Installation namespace, which is now handled explicitly. Final test writes a role, permission and assignment through the public service, observes peer authorization deny → native authenticated sync → allow → permission removal → sync → deny. All five outbound authorization tests pass.
- **Reset:** extended the existing remote Create/Join/reopen/reset HTTPS test. Before the fix, resetting the Create IdP consumer deleted six shared User/Client key rows. After the fix, every column of every `keys` row compares equal, and a restored second IdP consumer still authenticates the original User and signs a token. Existing Device reset/live-source checks remain.
- **Default local:** the original selectors-unset local Create test failed at Device readiness before the socket implementation. It now completes Create, authorized IdP administration, normal Device/Storage use, protected-material observations, reopen, restrictions/revocation and reset. No SQL config or injected fixture-only identity handle was added to its startup configuration.
- **IdP evaluator:** expanded local acceptance obtains an IdP-audience token and performs actual authorized administration. A mutation restoring remote-only permission injection produced HTTP 403, `Management permission evaluation is not configured`; restoring unconditional identity injection passes. Local Join also performs authorized IdP administration both fresh and after stopping the source and reopening the target.
- **Bootstrap:** native owner bootstrap and offline reopen pass with separate identity-policy and Device sessions. Fresh/offline standalone Join passes with imported identity grants and no imported password verifier.

### Validation

All Cargo commands used literal environment assignments:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_INCREMENTAL=0 RUST_MIN_STACK=67108864
```

Focused compilation/checks:

```sh
cargo test -p idp-service --no-default-features --features replica,std --lib security_sync -- --test-threads=2
cargo test -p unified-server --no-default-features --lib --no-run --message-format short
cargo check -p device-server -p idp-server -p management-server -p unified-server --all-targets --message-format short
cargo clippy -p device-service -p device-server -p idp-server -p unified-server --no-default-features --lib --no-deps -- -D warnings
cargo fmt --all -- --check
git --no-pager diff --check
```

All passed. IdP public projection tests: **5 passed**. Test compilation and execution were bounded separately. Final executable was `/home/nathan/code/rust/aicacia/of/target/debug/deps/unified_server-3151668e4144c159`, with `RUST_MIN_STACK=67108864` for each invocation:

| Filter (use `--exact` except the outbound module) | Result |
| --- | --- |
| `owner_composition_tests::create::production_independent_create_and_offline_reopen` | pass, **102.18s** final |
| `owner_composition_tests::join::production_independent_join_and_source_offline_reopen` | pass, **73.31s** final |
| `owner_composition_tests::remote_setup::remote_create_join_reopen_and_device_reset_preserve_shared_installation` | pass, **55.13s** final |
| `security_sync::outbound_tests --test-threads=1` | **5 passed**, **147.23s** final |
| `node_join_bootstrap_tests::owner_cutover_node_bootstrap_reopens_offline` | pass, **84.35s** |
| `security_sync_tests::authenticated_equal_peers_exchange_scoped_history_without_local_verifiers` | pass, **102.46s** |

Required broad validation was attempted after implementation:

```sh
timeout 180s env CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_INCREMENTAL=0 RUST_MIN_STACK=67108864 cargo hack test --feature-powerset --workspace --all-targets
timeout 180s env CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_INCREMENTAL=0 RUST_MIN_STACK=67108864 just crap
```

- Matrix passed combinations 1–16, including all 11 `idp-db` combinations, then failed at **17/177**, `cli --no-default-features`: Clap requires `std`. This is the documented inherited feature-wiring blocker; CLI dependencies were not changed. No full feature-matrix pass is claimed.
- CRAP exited **124 at 180 seconds** during coverage compilation (including app/server crates), with no score. No complete coverage/complexity qualification is claimed.
- An executable sweep filtered by `security_sync` hit its **300-second bound**. Individual outbound tests and the main authenticated history test were subsequently run successfully as above. Two broader interruption tests still fail after correcting their identity/Device fixture ownership: `desktop_bootstrap_orchestration_completes_both_public_contexts_and_retries_offline` at `receipt/pairing cannot allow ordinary DATA` (line 420), and `production_durable_transitions_preserve_imports_and_verified_readiness_on_reopen` at `actual imported trust survives without its journal` / `bootstrap requires exact approved Device` (line 524). These are **unclassified validation gaps**, not called inherited and not counted as passes. The base was not independently reproduced for those assertions. No unrelated production admission/readiness changes were made to silence them.
- Earlier exploratory runs encountered HTTPS recipient-registration/readiness timeouts and a whole-child 90-second bound; final focused lifecycle reruns passed at the documented larger bounds. Reliability/latency qualification is not implied.
- Tests use isolated mock protected providers, OS trust stubs and explicit stacks. Real OS-provider/default-stack, other-platform, GUI, mTLS rollout, WAN/container deployment and full workspace acceptance remain unqualified. Existing ofdb companion integration remains `59ac5a5`; no sibling changes or new sibling validation are claimed.

### Merger handoff

Implementation and integration-merge evidence will be appended after committing. Do not merge the original checkout from this worktree; the merger owns that operation.
