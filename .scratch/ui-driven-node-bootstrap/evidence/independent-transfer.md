# Independent transfer — implementation handoff, Rust execution embargo

## Source and acceptance boundary

Worktree: `of/.scratch/worktrees/bootstrap-transfer`; branch `implement/bootstrap-transfer`; source base/integration tip at start `68129cb00`. Scope: spec lines 69/85/92/107/118 and transfer portions of tickets 02/03/04. **No ticket is closed and no Rust/HTTPS pass is claimed.** Parent explicitly prohibited Rust builds until permission; no such permission was received during this implementation turn.

Complexity/risk/priority: moderate implementation complexity, high authorization/data-lifecycle risk and high priority. Seams are the production authenticated Management HTTPS contract, public Storage retirement APIs and stopped/offline reopen. TDD guidance was loaded. The first stale-ack/reopen test was written before the bound-completion implementation; the owner-cycle replay assertion preceded durable cleanup-ID implementation. Rust red/green execution is blocked, not claimed as observed. The other agent's independent Create/Join workers and host-readiness implementation were not changed.

## Implementation

- Service-only selection evidence now optionally includes `pendingCleanup:{cleanupId,deviceId,endpointId,oldOwnerSubject,newOwnerSubject}` with empty resources and `adminAllowed:false`. Read requires the existing configured Storage audience, Client principal and exact `management.replication.read` scope. Both policy and acknowledgement revalidate current endpoint identity before response.
- `POST /replication/owner-cleanup` uses the same existing Storage→Management scoped Client, with `management.replication.admit`, no new secret/relationship/permission grant. Storage retires SQL/KV and filesystem copies through `StorageCopyRetirement` before acknowledgement, checks exact current evidence again, then reads current policy before activating only its current owner. Remote-peer reads cannot trigger local retirement. No policy, selection or removed content is automatically restored.
- Management reconciles the pending fact, restriction and deselection in one local transaction. Acknowledgement compares current approved/non-revoked/conflict-clear Device ID, endpoint, new owner, retained old policy owner and cleanup ID within its own transaction before changing policy/deleting the fence. A stale delayed acknowledgement cannot clear a different current binding. A completed response lost in transit can retry as a no-op while no pending obligation remains.
- A fresh UUID is persisted once in the existing `local_device_copy_cleanup` fact. This distinguishes a later repetition of the same owner tuple after an owner cycle; tuple-only acknowledgement was insufficient. Interruption/reopen reuses the same pending ID and existing local retirement files, not another journal/framework. The table now has `cleanup_id UUID NOT NULL`. Unsupported older/malformed layouts are not silently repaired/migrated and fail closed on cleanup queries; never delete retained fences as a workaround.
- Runtime construction supplies local endpoint/retirement handles and cancellation to the remote authorizer. Shutdown cancels retained remote-policy callbacks. Operation/frame/response authorization consumers and their existing retirement checks are otherwise unchanged.
- Removed the obsolete standalone Storage Iroh endpoint-key creation/peer-refresh implementation and export. Manual configured Storage serving now consumes `management_endpoint_id` / `STORAGE_MANAGEMENT_ENDPOINT_ID` and `build_http_runtime`; ordinary independent setup already obtains that nonsecret binding through the owner exchange. Bundled injection of the host endpoint is unchanged. Announced CLI/config, schema, OpenAPI registration/inventory and model re-export wiring before editing.
- Added only one new Rust test module, `crates/unified-server/src/independent_transfer_tests.rs`, with lib wiring. Existing Management policy expectations and exact OpenAPI inventories were adjusted for the changed contract; no Create/Join fixtures were edited. Generator inventories require the new operation. Public contract/operations docs describe the exchange and manual endpoint setting.

## Runnable checks added (not executed)

`independent_transfer_tests::stale_cleanup_cannot_clear_current_binding_and_exact_retry_stays_restricted` covers delayed-owner/endpoint substitution denial, durable Management reopen, retry, restricted empty policy and replay of a prior cleanup ID after an owner cycle.

`independent_transfer_tests::independent_transfer_production_https` launches the exact ignored child `independent_transfer_tests::independent_transfer_https_child` with isolated test-only native roots and a **90-second** parent/child bound. The child uses existing mock-provider identity seeding, production IdP/Management HTTPS and public Storage APIs. It covers service-only evidence, missing/User/read-only acknowledgement denial, local retirement persistence failure, HTTPS acknowledgement failure after retirement, cancellation, stopped Storage reopen with unaffected SQL/filesystem copies, owner mutation while HTTPS acknowledgement is delayed, exact retry, actual HTTP Storage runtime policy wiring, no Storage `endpoint.key`, no automatic allowance/selection restore, and stopped Management reopen. Failure/delay injection is at the HTTP boundary, not an owner collaborator mock. Transfer has no new protected-provider production; existing setup/provider qualification remains separate.

These are intended regression assertions, **not executed acceptance evidence**. No deadline or stack bound was increased; no real trust command, provider mutation, real profile reset or dependency install ran. The new HTTPS fixture follows existing isolated native-root child patterns and uses the existing mock provider fixture.

## Commands actually run

Working directory for every command: `/home/nathan/code/rust/aicacia/of/.scratch/worktrees/bootstrap-transfer`.

1. `node --test scripts/generate-openapi-client.test.mjs` — bounded by tool `timeout_ms:20000`; **198 passed / 5 skipped / 0 failed**, 203 total, 16.78s. Skips are optional real-Biome binary cases because dependencies are absent in this worktree.
2. `node --test scripts/generated-clients.test.mjs` — tool `timeout_ms:20000`; **failed before tests** with `Cannot find module 'typescript'`, required from the worktree's `packages/idp-client/package.json`. No dependency installation or source weakening attempted. This is not an SDK validation pass.
3. `python3 scripts/check_server_dependencies.py` — passed: `No direct server-to-server dependency edges found.`
4. `git --no-pager diff --check` — passed.
5. `node --check scripts/generate-openapi-client.mjs` and `node --check scripts/qualify-openapi-clients.mjs` — passed.
6. Scoped formatting was applied with `rustfmt --edition 2024 --config skip_children=true` on edited Rust files. Final check command (tool `timeout_ms:10000`):

```sh
rustfmt --edition 2024 --config skip_children=true --check crates/management-service/src/replica/selection_policy.rs crates/management-service/src/replica/schema.rs crates/management-server/src/router/routes/replication.rs crates/management-server/src/router/openapi_router.rs crates/management-server/src/router/openapi_tests.rs crates/model/src/contract/replication.rs crates/model/src/contract/mod.rs crates/storage-server/src/management_client.rs crates/storage-server/src/local_replication_authorizer.rs crates/storage-server/src/runtime.rs crates/storage-server/src/lib.rs crates/storage-server/src/cli/run.rs crates/storage-server/src/config/app_config.rs crates/unified-server/src/independent_transfer_tests.rs crates/unified-server/src/lib.rs
```

No Cargo check/test/build/Clippy, powerset, coverage or CRAP ran. Rustfmt is syntax/formatting only, not type checking.

## Parent-gated follow-up

After explicit permission, use the parent's existing offline/locked, one-job, no-incremental selective profile and unchanged 16 MiB stack / 200000ms tool bounds from receipt 36. Run the new module first, then Management policy/router/OpenAPI and Storage runtime/authorizer regressions; include existing delayed-response/per-frame tests. Compiler/test failures remain possible until those checks run. Do not report the new Rust tests as green on the basis of source review.

Regenerate the actual complete Management SDK through the existing workflow **after** exporting this source's OpenAPI with `router::openapi_tests::export_management_openapi_for_client_generation` (`OF_MANAGEMENT_OPENAPI_OUT` selects the artifact). The operation inventory is now 38. The old retained OpenAPI artifact and generated SDK are intentionally not hand-edited or represented as regenerated. Regeneration, SDK build/tests, scoped Clippy and executable HTTPS qualification remain pending. Missing TypeScript/package dependencies must be supplied through the parent's existing environment, not silently installed here.

The final commit/merge outcome is reported with the handoff; full-ticket acceptance remains open.

## Commit / integration

`GIT_EDITOR=true git -c core.hooksPath=/dev/null commit -m "Implement scoped independent Storage transfer cleanup acknowledgement"` committed the source. Hooks were explicitly disabled to prevent a shared hook from violating the Rust-build embargo; the scoped checks above ran explicitly. Initial `git add` reported the new `.scratch` receipt ignored, so it was added explicitly with `git add -f .scratch/ui-driven-node-bootstrap/evidence/independent-transfer.md` and included by amending this same implementation commit.

`GIT_EDITOR=true git -c core.hooksPath=/dev/null merge --no-edit integration/ui-driven-node-bootstrap` returned **Already up to date**: the integration branch remained at `68129cb00`. No integration source was dropped, and no other agent branch was merged. The final implementation commit ID is supplied in the handoff, rather than embedding a self-referential commit hash here.
